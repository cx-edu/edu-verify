use sha3::{Digest, Keccak256};
use snark_verifier_sdk::snark_verifier::halo2_base::halo2_proofs::halo2curves::bn256::Fr;

pub fn compress_g1(data: &[Vec<u8>], rand: Fr) -> Vec<u8> {
    let mut res = ark_bn254::G1Projective::default();
    let rand_ark: ark_bn254::Fr = unsafe { std::mem::transmute(rand) };
    for d in data.iter().rev() {
        let d = unsafe {
            std::slice::from_raw_parts(d.as_ptr() as *const u8 as *const ark_bn254::G1Projective, 1)
        };
        res *= rand_ark;
        res += d[0];
    }

    unsafe { std::mem::transmute::<_, [u8; 96]>(res) }.to_vec()
}

// pub fn compress_proof(data: &[Proof], rand: Fr) -> Proof {
//     let data_g1: Vec<G1> = data.iter().map(|d| d.0).collect();

//     Proof(compress_g1(&data_g1, rand))
// }

// pub fn compress_commit(data: &[Commit], rand: Fr) -> Commit {
//     let data_g1: Vec<G1> = data.iter().map(|d| d.0).collect();

//     Commit(compress_g1(&data_g1, rand))
// }

pub fn compress_fr(data: &[Fr], rand: Fr) -> Fr {
    let mut res = Fr::zero();

    for d in data.iter().rev() {
        res *= rand;
        res += d;
    }

    res
}

pub fn read_as<T: Clone>(path: &str) -> Vec<T> {
    let bytes = std::fs::read(path).expect("path not exist");

    assert_eq!(bytes.len() % size_of::<T>(), 0);

    let data = unsafe {
        std::slice::from_raw_parts(bytes.as_ptr() as *const T, bytes.len() / size_of::<T>())
    };

    data.to_vec()
}

pub fn hash_to_u64(hash: &[u8]) -> u64 {
    let hash = Keccak256::digest(hash);

    u64::from_le_bytes(hash.to_vec()[0..8].try_into().unwrap())
}
