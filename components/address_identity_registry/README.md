# Address Identity Registry (AIR)

The authenticated map used in the AIR is implemented by a **Sparse Merkle Tree**, or SMT.

There are four roles implemented in the system:
1. **User**, which are the holders of credentials and identity
2. **Issuer** - issuers of credential, eg banks, passport agencies.
3. **Ledger** - The database which holds the map from user address to credential list
4. **Verifier** - A party that interacts with the user, and verifies the users credentials

The Ledger holds the AIR, amongst other things. It must be able to add an (address, credentials) pair to an
address and subsequently provide that address as well as a (Merkle) proof of inclusion or non-inclusion.

There reader should be aware that in the discussion below, there are two kinds of signatures, **ACSignature** and 
**ACRevealSig**. The latter, the result of calling reveal or commit functions, has three fields: sig, a randomized
**ACSignature**, pok: a proof of knowledge, and rnd, the randomness used to generate it.

## Setup
1. Credential issuers are initialized with the number of attributes they can handle.
   The supported attributes are a feature of the issuer; e.g., United States Department of State issues
   U.S. passports, and they decide what's in them. Tricky issue: credential issuers may issue more
   than one kind of credential, with varying numbers of fields.
2. User contacts an Issuer (which one?) and acquires issuer public key (Txn: User -> Issuer -> User)
3. The user generates its own user key pair using the issuer public key.

## Credential Generation
1. The User asks the Issuer to **ac_sign** a set of credentials the user provides and receives a signature back.
   (Txn : User -> Issuer -> User)
2. The User generates a commitment to the signed attribute values using **ac_commit** and stores both the commitment
   and the *commitment key* used to generate the it, in a User wallet. The User generates a unique address (how?),
   and asks the Ledger to store the commitment in the AIR at that address. (Txn: User -> Ledger)

## Credential Verificration
1. A Verifier asks the User to selectively reveal some attributes. (Txn: Verifier sends attributes -> User)
2. The User runs **ac_open_commitment** using the committed signature and *commitment key* stored in his wallet, and the attributes
   supplied by the Verifier. The result is a reveal_sig: ACRevealSig. The User sends the proof of knowledge, reveal_sig.pok,
   and the address, back to the verifier (Txn: User sends address, pok -> Verifier)
3. The Verifier queries the SMT on the Ledger using the address, and retrieves the signature and a Merkle proof
   of (non)inclusion. The Verifier checks the Merkle Proof, and if it is successful, runs **ac_verify** with the signature,
   the attributes, and the proof of knowledge.
   If neither the Merkle proof nor the **ac_verify** fail, the Verifier reports success, otherwise it reports failure.

## Remaining issues
1. A given user has more than one set of credentials that need to be on AIR, say {passport, driver's license, bank 
accounts, etc.}. While it's easy enough to store anything (the SMT stores "blobs" of variable length data) I think
it's better that every address only store one kind of credential, otherwise retrieving the data at an address would
give all of the credentials. So I assume the user manages all of their credentials, and the address is something like
a hash of sk and credential id. Given this, it seems that having some sort of `credential kind` as an explicit concept
would make sense.

2. When the Verifier completes, is there another party to which it is reporting the results?