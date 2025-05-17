use sha3::{Digest, Keccak256, Sha3_256};

pub fn verify(data: String, ex_commit: String, ex_proof: String) -> bool {
    // let mut hasher = Keccak256::new();
    // hasher.update(&data);
    // let hash = hasher.finalize();

    // let proof = commit(hash.to_vec());

    // let commit_data_res = hex::encode(hash) == ex_commit;
    // let proof_res = proof == ex_proof;

    // commit_data_res && proof_res

    todo!()
}

pub fn verify_proof(
    data: &[String],
    commit_vec: &[String],
    proof: String,
    zk_proof: String,
    random: String,
) -> bool {
    // let mut hasher = Keccak256::new();
    // for item in data {
    //     hasher.update(&item);
    // }
    // let hash = hasher.finalize();

    // let proof_data = commit(hash.to_vec());

    // proof_data == proof

    true
}
