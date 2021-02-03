use actix_cors::Cors;
use actix_web::{error, middleware, web, App, HttpServer};
use actix_service::Service;
use futures::Future;
use ledger::data_model::errors::PlatformError;
use ledger::data_model::{
    b64dec, AssetTypeCode, DefineAsset, IssuerPublicKey, KVBlind, KVHash, TxOutput,
    TxnSID, TxoSID, XfrAddress,
};
use ledger::{error_location, inp_fail, ser_fail};
use ledger_api_service::RestfulArchiveAccess;
use log::info;
use metrics::{Unit, KeyData, Key as MetricsKey, IntoF64, Recorder};
use query_server::{QueryServer, MetricsRenderer};
use sparse_merkle_tree::Key;
use std::collections::HashSet;
use std::io;
use std::marker::{Send, Sync};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use utils::{actix_get_request, actix_post_request, NetworkRoute};
use zei::serialization::ZeiFromToBytes;
use zei::xfr::sig::XfrPublicKey;
use zei::xfr::structs::OwnerMemo;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::ops::Deref;

/// Returns the git commit hash and commit date of this build
fn version() -> actix_web::Result<String> {
    Ok(concat!(
        "Build: ",
        env!("VERGEN_SHA_SHORT"),
        " ",
        env!("VERGEN_BUILD_DATE")
    )
    .into())
}

// Queries the status of a transaction by its handle. Returns either a not committed message or a
// serialized TxnStatus.
fn get_address<T, U>(
    data: web::Data<Arc<RwLock<QueryServer<T, U>>>>,
    info: web::Path<u64>,
) -> Result<String, actix_web::error::Error>
where
    T: RestfulArchiveAccess,
    U: MetricsRenderer
{
    let query_server = data.read().unwrap();
    let address_res = query_server.get_address_of_sid(TxoSID(*info));
    let res;
    if let Some(address) = address_res {
        res = serde_json::to_string(&address)?;
    } else {
        res = format!("No utxo {} found. Please retry with a new utxo.", &info);
    }
    Ok(res)
}

type CustomDataResult = (Vec<u8>, KVHash);

// Returns custom data at a given location
fn get_custom_data<T, U>(
    data: web::Data<Arc<RwLock<QueryServer<T, U>>>>,
    info: web::Path<String>,
) -> actix_web::Result<web::Json<Option<CustomDataResult>>, actix_web::error::Error>
where
    T: RestfulArchiveAccess,
    U: MetricsRenderer
{
    let query_server = data.read().unwrap();
    let key = Key::from_base64(&*info)
        .map_err(|_| actix_web::error::ErrorBadRequest("Could not deserialize Key"))?;
    Ok(web::Json(query_server.get_custom_data(&key).cloned()))
}

// Returns the owner memo required to decrypt the asset record stored at given index, if it exists.
fn get_owner_memo<T, U>(
    data: web::Data<Arc<RwLock<QueryServer<T, U>>>>,
    info: web::Path<u64>,
) -> actix_web::Result<web::Json<Option<OwnerMemo>>, actix_web::error::Error>
where
    T: RestfulArchiveAccess,
    U: MetricsRenderer
{
    let query_server = data.read().unwrap();
    Ok(web::Json(
        query_server.get_owner_memo(TxoSID(*info)).cloned(),
    ))
}

// Submits custom data to be stored by the query server. The request will fail if the hash of the
// data doesn't match the commitment stored by the ledger.
fn store_custom_data<T, U>(
    data: web::Data<Arc<RwLock<QueryServer<T, U>>>>,
    body: web::Json<(String, Vec<u8>, Option<KVBlind>)>,
) -> actix_web::Result<(), actix_web::error::Error>
where
    T: RestfulArchiveAccess + Sync + Send,
    U: MetricsRenderer
{
    let (key, custom_data, blind) = body.into_inner();
    let key = Key::from_base64(&key)
        .map_err(|_| actix_web::error::ErrorBadRequest("Could not deserialize Key"))?;
    let mut query_server = data.write().unwrap();
    query_server
        .add_to_data_store(&key, &custom_data, blind.as_ref())
        .map_err(|e| error::ErrorBadRequest(format!("{}", e)))?;
    Ok(())
}
// Returns an array of the utxo sids currently spendable by a given address
fn get_owned_utxos<T, U>(
    data: web::Data<Arc<RwLock<QueryServer<T, U>>>>,
    info: web::Path<String>,
) -> actix_web::Result<web::Json<HashSet<TxoSID>>>
where
    T: RestfulArchiveAccess + Sync + Send,
    U: MetricsRenderer
{
    // Convert from basee64 representation
    let key: XfrPublicKey = XfrPublicKey::zei_from_bytes(
        &b64dec(&*info)
            .map_err(|_| error::ErrorBadRequest("Could not deserialize public key"))?,
    )
    .map_err(|_| error::ErrorBadRequest("Could not deserialize public key"))?;
    let query_server = data.read().unwrap();
    let sids = query_server.get_owned_utxo_sids(&XfrAddress { key });
    Ok(web::Json(sids.cloned().unwrap_or_default()))
}

// Returns rendered metrics
fn get_metrics<T, U>(
    data: web::Data<Arc<RwLock<QueryServer<T, U>>>>,
    info: web::Path<()>,
) -> actix_web::Result<String>
    where
        T: RestfulArchiveAccess + Sync + Send,
        U: MetricsRenderer
{
    let query_server= data.read().unwrap();

    Ok(query_server.render())
}

pub enum QueryServerRoutes {
    GetAddress,
    GetOwnerMemo,
    GetOwnedUtxos,
    StoreCustomData,
    GetCustomData,
    GetCreatedAssets,
    GetTracedAssets,
    GetIssuedRecords,
    GetIssuedRecordsByCode,
    GetRelatedTxns,
    GetRelatedXfrs,
    Version,
}

impl NetworkRoute for QueryServerRoutes {
    fn route(&self) -> String {
        let endpoint = match *self {
            QueryServerRoutes::GetAddress => "get_address",
            QueryServerRoutes::GetRelatedTxns => "get_related_txns",
            QueryServerRoutes::GetRelatedXfrs => "get_related_xfrs",
            QueryServerRoutes::GetOwnedUtxos => "get_owned_utxos",
            QueryServerRoutes::GetOwnerMemo => "get_owner_memo",
            QueryServerRoutes::StoreCustomData => "store_custom_data",
            QueryServerRoutes::GetCustomData => "get_custom_data",
            QueryServerRoutes::GetCreatedAssets => "get_created_assets",
            QueryServerRoutes::GetTracedAssets => "get_traced_assets",
            QueryServerRoutes::GetIssuedRecords => "get_issued_records",
            QueryServerRoutes::GetIssuedRecordsByCode => "get_issued_records_by_code",
            QueryServerRoutes::Version => "version",
        };
        "/".to_owned() + endpoint
    }
}

// Returns the list of assets created by a public key
fn get_created_assets<T, U>(
    data: web::Data<Arc<RwLock<QueryServer<T, U>>>>,
    info: web::Path<String>,
) -> actix_web::Result<web::Json<Vec<DefineAsset>>>
where
    T: RestfulArchiveAccess + Sync + Send,
    U: MetricsRenderer
{
    // Convert from base64 representation
    let key: XfrPublicKey = XfrPublicKey::zei_from_bytes(
        &b64dec(&*info)
            .map_err(|_| error::ErrorBadRequest("Could not deserialize public key"))?,
    )
    .map_err(|_| error::ErrorBadRequest("Could not deserialize public key"))?;
    let query_server = data.read().unwrap();
    let assets = query_server.get_created_assets(&IssuerPublicKey { key });
    Ok(web::Json(assets.cloned().unwrap_or_default()))
}

// Returns the list of assets traced by a public key
fn get_traced_assets<T, U>(
    data: web::Data<Arc<RwLock<QueryServer<T, U>>>>,
    info: web::Path<String>,
) -> actix_web::Result<web::Json<Vec<AssetTypeCode>>>
where
    T: RestfulArchiveAccess + Sync + Send,
    U: MetricsRenderer
{
    // Convert from base64 representation
    let key: XfrPublicKey = XfrPublicKey::zei_from_bytes(
        &b64dec(&*info)
            .map_err(|_| error::ErrorBadRequest("Could not deserialize public key"))?,
    )
    .map_err(|_| error::ErrorBadRequest("Could not deserialize public key"))?;
    let query_server = data.read().unwrap();
    let assets = query_server.get_traced_assets(&IssuerPublicKey { key });
    Ok(web::Json(assets.cloned().unwrap_or_default()))
}

// Returns the list of records issued by a public key
fn get_issued_records<T, U>(
    data: web::Data<Arc<RwLock<QueryServer<T, U>>>>,
    info: web::Path<String>,
) -> actix_web::Result<web::Json<Vec<(TxOutput, Option<OwnerMemo>)>>>
where
    T: RestfulArchiveAccess + Sync + Send,
    U: MetricsRenderer
{
    // Convert from base64 representation
    let key: XfrPublicKey = XfrPublicKey::zei_from_bytes(
        &b64dec(&*info)
            .map_err(|_| error::ErrorBadRequest("Could not deserialize public key"))?,
    )
    .map_err(|_| error::ErrorBadRequest("Could not deserialize public key"))?;
    let query_server = data.read().unwrap();
    let records = query_server.get_issued_records(&IssuerPublicKey { key });
    Ok(web::Json(records.unwrap_or_default()))
}

// Returns the list of records issued by a token code
fn get_issued_records_by_code<T, U>(
    data: web::Data<Arc<RwLock<QueryServer<T, U>>>>,
    info: web::Path<String>,
) -> actix_web::Result<web::Json<Vec<(TxOutput, Option<OwnerMemo>)>>>
where
    T: RestfulArchiveAccess + Sync + Send,
    U: MetricsRenderer
{
    let query_server = data.read().unwrap();
    if let Ok(token_code) = AssetTypeCode::new_from_base64(&*info) {
        if let Some(records) = query_server.get_issued_records_by_code(&token_code) {
            Ok(web::Json(records))
        } else {
            Err(actix_web::error::ErrorNotFound(
                "Specified asset definition does not currently exist.",
            ))
        }
    } else {
        Err(actix_web::error::ErrorBadRequest(
            "Invalid asset definition encoding.",
        ))
    }
}

// Returns the list of transations associated with a given ledger address
fn get_related_txns<T, U>(
    data: web::Data<Arc<RwLock<QueryServer<T, U>>>>,
    info: web::Path<String>,
) -> actix_web::Result<web::Json<HashSet<TxnSID>>>
where
    T: RestfulArchiveAccess + Sync + Send,
    U: MetricsRenderer
{
    // Convert from base64 representation
    let key: XfrPublicKey = XfrPublicKey::zei_from_bytes(
        &b64dec(&*info)
            .map_err(|_| error::ErrorBadRequest("Could not deserialize public key"))?,
    )
    .map_err(|_| error::ErrorBadRequest("Could not deserialize public key"))?;
    let query_server = data.read().unwrap();
    let records = query_server.get_related_transactions(&XfrAddress { key });
    Ok(web::Json(records.cloned().unwrap_or_default()))
}

// Returns the list of transfer transations associated with a given asset
fn get_related_xfrs<T, U>(
    data: web::Data<Arc<RwLock<QueryServer<T, U>>>>,
    info: web::Path<String>,
) -> actix_web::Result<web::Json<HashSet<TxnSID>>>
where
    T: RestfulArchiveAccess + Sync + Send,
    U: MetricsRenderer
{
    let query_server = data.read().unwrap();
    if let Ok(token_code) = AssetTypeCode::new_from_base64(&*info) {
        if let Some(records) = query_server.get_related_transfers(&token_code) {
            Ok(web::Json(records.clone()))
        } else {
            Err(actix_web::error::ErrorNotFound(
                "Specified asset definition does not currently exist.",
            ))
        }
    } else {
        Err(actix_web::error::ErrorBadRequest(
            "Invalid asset definition encoding.",
        ))
    }
}

pub struct QueryApi {
    web_runtime: actix_rt::SystemRunner,
}

impl QueryApi {
    pub fn create<T, U>(
        query_server: Arc<RwLock<QueryServer<T, U>>>,
        host: &str,
        port: &str,
    ) -> io::Result<QueryApi>
    where
        T: 'static + RestfulArchiveAccess + Sync + Send,
        U: 'static + MetricsRenderer + Sync + Send
    {
        let web_runtime = actix_rt::System::new("findora API");

        HttpServer::new(move || {
            App::new()
                .wrap(middleware::Logger::default())
                .wrap(Cors::new().supports_credentials())
                .data(query_server.clone())
                .wrap_fn(|req, srv| {
                    let start = Instant::now();
                    srv.call(req).map(move |res| {
                        // TODO: parse req and get route name for key_data to be req specific
                        let recorder = metrics::recorder();
                        let duration = start.elapsed();
                        let key_data = KeyData::from_name("query.histogram");
                        let key = MetricsKey::from(key_data);
                        recorder.record_histogram(key, duration.into_f64());

                        // let h = handle.render();
                        // let h: PrometheusHandle = recorder.into();
                        // println!("{}", handle.render());

                        // println!("{}", handle.render());
                        println!("{}", duration.as_micros());
                        // println!("{}", h);

                        res
                    })
                })
                .route(
                    &QueryServerRoutes::GetAddress.with_arg_template("txo_sid"),
                    web::get().to(get_address::<T, U>),
                )
                .route(
                    &QueryServerRoutes::GetOwnedUtxos.with_arg_template("address"),
                    web::get().to(get_owned_utxos::<T, U>),
                )
                .route(
                    &QueryServerRoutes::GetOwnerMemo.with_arg_template("txo_sid"),
                    web::get().to(get_owner_memo::<T, U>),
                )
                .route(
                    &QueryServerRoutes::GetRelatedTxns.with_arg_template("address"),
                    web::get().to(get_related_txns::<T, U>),
                )
                .route(
                    &QueryServerRoutes::GetRelatedXfrs.with_arg_template("asset_token"),
                    web::get().to(get_related_xfrs::<T, U>),
                )
                .route(
                    &QueryServerRoutes::GetCreatedAssets.with_arg_template("address"),
                    web::get().to(get_created_assets::<T, U>),
                )
                .route(
                    &QueryServerRoutes::GetTracedAssets.with_arg_template("address"),
                    web::get().to(get_traced_assets::<T, U>),
                )
                .route(
                    &QueryServerRoutes::GetIssuedRecords.with_arg_template("address"),
                    web::get().to(get_issued_records::<T, U>),
                )
                .route(
                    &QueryServerRoutes::GetIssuedRecordsByCode
                        .with_arg_template("asset_token"),
                    web::get().to(get_issued_records_by_code::<T, U>),
                )
                .route(
                    &QueryServerRoutes::StoreCustomData.route(),
                    web::post().to(store_custom_data::<T, U>),
                )
                .route(
                    &QueryServerRoutes::GetCustomData.with_arg_template("key"),
                    web::get().to(get_custom_data::<T, U>),
                )
                .route(&QueryServerRoutes::Version.route(), web::get().to(version))
                .route(&String::from("/metrics"), web::get().to(get_metrics::<T, U>))
        })
        .bind(&format!("{}:{}", host, port))?
        .start();

        info!("Query server started");

        Ok(QueryApi { web_runtime })
    }

    // call from a thread; this will block.
    pub fn run(self) -> io::Result<()> {
        self.web_runtime.run()
    }
}

// Trait for rest clients that can access the query server
pub trait RestfulQueryServerAccess {
    fn store_custom_data(
        &mut self,
        data: &dyn AsRef<[u8]>,
        key: &Key,
        blind: Option<KVBlind>,
    ) -> Result<(), PlatformError>;

    fn fetch_custom_data(&self, key: &Key) -> Result<Vec<u8>, PlatformError>;

    fn get_owner_memo(&self, txo_sid: u64) -> Result<Option<OwnerMemo>, PlatformError>;
}

// Unimplemented until I can figure out a way to force the mock server to get new data (we can do
// this with a new endpoint)
pub struct MockQueryServerClient();

impl RestfulQueryServerAccess for MockQueryServerClient {
    fn store_custom_data(
        &mut self,
        _data: &dyn AsRef<[u8]>,
        _key: &Key,
        _blind: Option<KVBlind>,
    ) -> Result<(), PlatformError> {
        unimplemented!();
    }

    fn fetch_custom_data(&self, _key: &Key) -> Result<Vec<u8>, PlatformError> {
        unimplemented!();
    }

    fn get_owner_memo(&self, _txo_sid: u64) -> Result<Option<OwnerMemo>, PlatformError> {
        unimplemented!();
    }
}

pub struct ActixQueryServerClient {
    port: usize,
    host: String,
    protocol: String,
    client: reqwest::blocking::Client,
}

impl ActixQueryServerClient {
    pub fn new(port: usize, host: &str, protocol: &str) -> Self {
        ActixQueryServerClient {
            port,
            host: String::from(host),
            protocol: String::from(protocol),
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl RestfulQueryServerAccess for ActixQueryServerClient {
    fn store_custom_data(
        &mut self,
        data: &dyn AsRef<[u8]>,
        key: &Key,
        blind: Option<KVBlind>,
    ) -> Result<(), PlatformError> {
        let query = format!(
            "{}://{}:{}{}",
            self.protocol,
            self.host,
            self.port,
            QueryServerRoutes::StoreCustomData.route()
        );
        actix_post_request(
            &self.client,
            &query,
            Some(&(key, data.as_ref().to_vec(), blind)),
        )
        .map_err(|_| inp_fail!())?;
        Ok(())
    }

    fn fetch_custom_data(&self, key: &Key) -> Result<Vec<u8>, PlatformError> {
        let b64key = key.to_base64();
        let query = format!(
            "{}://{}:{}{}",
            self.protocol,
            self.host,
            self.port,
            QueryServerRoutes::GetCustomData.with_arg(&b64key)
        );
        let text = actix_get_request(&self.client, &query).map_err(|_| inp_fail!())?;
        Ok(serde_json::from_str::<Vec<u8>>(&text).map_err(|_| ser_fail!())?)
    }

    fn get_owner_memo(&self, txo_sid: u64) -> Result<Option<OwnerMemo>, PlatformError> {
        let query = format!(
            "{}://{}:{}{}",
            self.protocol,
            self.host,
            self.port,
            QueryServerRoutes::GetOwnerMemo.with_arg(&txo_sid)
        );
        let text = actix_get_request(&self.client, &query).map_err(|_| inp_fail!())?;
        Ok(serde_json::from_str::<Option<OwnerMemo>>(&text).map_err(|_| ser_fail!())?)
    }
}
