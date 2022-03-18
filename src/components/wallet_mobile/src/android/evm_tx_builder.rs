use crate::rust::account::EVMTransactionBuilder;
use jni::objects::{JClass, JString};
use jni::sys::{jlong, jstring};
use jni::JNIEnv;
use zei::xfr::sig::XfrKeyPair;

use super::{jStringToString, parseU64};

#[no_mangle]
/// # Safety
/// Construct a EVM Transaction that transfer account balance to UTXO.
/// @param {unsigned long long} amount - Amount to transfer.
/// @param {XfrKeyPair} fra_kp - Fra key pair.
/// @param {String} address - EVM address.
/// @param {String} eth_phrase - The account mnemonic.
/// @param {String} nonce - Json encoded U256(256 bits unsigned integer).
pub unsafe extern "system" fn Java_com_findora_JniApi_transfer_from_account_evmTransactionBuilder(
    env: JNIEnv,
    _: JClass,
    amount: JString,
    address: JString,
    fra_kp: jlong,
    eth_phrase: JString,
    nonce: JString,
) -> jlong {
    let address = {
        let a = jStringToString(env, address);
        if a.is_empty() {
            None
        } else {
            Some(a)
        }
    };

    let eth_phrase = {
        let a = jStringToString(env, eth_phrase);
        if a.is_empty() {
            None
        } else {
            Some(a)
        }
    };

    let nonce = serde_json::from_str(&jStringToString(env, nonce)).unwrap();

    let amount = parseU64(env, amount);

    let fra_kp = &*(fra_kp as *mut XfrKeyPair);

    let tx = EVMTransactionBuilder::new_transfer_from_account(
        amount,
        address,
        fra_kp,
        eth_phrase,
        nonce,
    )
    .unwrap();
    Box::into_raw(Box::new(tx)) as jlong
}

#[no_mangle]
/// # Safety
/// Extracts the serialized form of the evm transaction.
pub unsafe extern "system" fn Java_com_findora_JniApi_evmTransactionBuilderTransaction(
    env: JNIEnv,
    _: JClass,
    builder: jlong,
) -> jstring {
    let builder = &*(builder as *mut EVMTransactionBuilder);
    let output = env
        .new_string(builder.serialized_transaction_base64())
        .expect("Couldn't create java string!");
    output.into_inner()
}

#[no_mangle]
/// # Safety
///Free the memory.
///**Danger**: do this make the builder a dangling pointer.
pub unsafe extern "system" fn Java_com_findora_JniApi_free_evmTransactionBuilder(
    _env: JNIEnv,
    _: JClass,
    builder: jlong,
) {
    let _ = Box::from_raw(builder as *mut EVMTransactionBuilder);
}
