use std::mem::{transmute, transmute_copy};

use crate::services::shplonk_inner::Shplonk;
use ark_ff::One;
use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use snark_verifier_sdk::snark_verifier::halo2_base::halo2_proofs::halo2curves::bn256::{
    Fr, G1, G2,
};

const MAX_DEGREE: usize = 1 << 16;
const G1_BYTES: [u8; 96] = [
    190, 63, 28, 202, 99, 84, 194, 148, 207, 100, 192, 152, 222, 162, 45, 4, 0, 158, 148, 183, 219,
    251, 107, 244, 110, 120, 59, 126, 79, 212, 221, 42, 86, 41, 155, 94, 102, 129, 225, 218, 98,
    164, 85, 100, 56, 4, 84, 129, 32, 95, 167, 202, 165, 137, 27, 151, 176, 107, 55, 154, 69, 159,
    162, 23, 157, 13, 143, 197, 141, 67, 93, 211, 61, 11, 199, 245, 40, 235, 120, 10, 44, 70, 121,
    120, 111, 163, 110, 102, 47, 223, 7, 154, 193, 119, 10, 14,
];
const G2_BYTES: [u8; 192] = [
    148, 147, 173, 125, 76, 195, 200, 182, 5, 243, 101, 212, 203, 26, 202, 173, 89, 68, 158, 62,
    88, 143, 196, 67, 129, 0, 37, 58, 116, 68, 60, 32, 36, 243, 93, 222, 49, 5, 94, 168, 143, 181,
    200, 27, 171, 2, 247, 218, 65, 71, 101, 153, 79, 77, 171, 180, 178, 194, 25, 40, 52, 144, 97,
    45, 96, 82, 147, 63, 35, 175, 35, 22, 143, 73, 181, 159, 80, 0, 6, 236, 138, 22, 245, 113, 6,
    239, 22, 138, 103, 221, 255, 147, 5, 60, 219, 4, 126, 213, 124, 244, 128, 253, 75, 85, 101, 44,
    252, 81, 90, 37, 28, 227, 221, 221, 58, 168, 239, 206, 215, 4, 187, 177, 29, 244, 135, 81, 217,
    27, 4, 221, 53, 178, 179, 197, 180, 4, 165, 228, 88, 4, 155, 106, 132, 34, 18, 183, 28, 96,
    143, 255, 32, 235, 46, 156, 124, 154, 213, 213, 227, 15, 213, 209, 36, 46, 46, 186, 53, 46, 73,
    193, 134, 24, 247, 205, 92, 143, 191, 70, 27, 100, 17, 93, 15, 125, 92, 48, 121, 134, 148, 107,
    98, 23,
];

lazy_static! {
    static ref SHPLONK_INSTANCE: Shplonk<ark_bn254::Bn254> = {
        let mut shplonk = unsafe {Shplonk::new(transmute(G1_BYTES), transmute(G2_BYTES), MAX_DEGREE)};

        shplonk.setup(ark_bn254::Fr::one());

        shplonk
    };
    pub static ref DOMAIN: Vec<Fr> = {
        let domain = Radix2EvaluationDomain::<ark_bn254::Fr>::new(MAX_DEGREE).unwrap();

        domain
            .elements()
            .map(|e| unsafe { transmute(e) })
            .collect::<Vec<Fr>>()
    };
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Commit(pub G1);

#[derive(Debug, Serialize, Deserialize)]
pub struct Proof(pub G1);

pub fn shplonk_lagrange_commit(evals: &[Fr]) -> Commit {
    let evals_ark: &[ark_bn254::Fr] = unsafe { transmute_copy(&evals) };

    let res: ark_bn254::G1Projective = (*SHPLONK_INSTANCE).commit_lagrange_g1(evals_ark);

    Commit(unsafe { transmute(res) })
}

pub fn shplonk_lagrange_commit_g2(evals: &[Fr]) -> G2 {
    let evals_ark: &[ark_bn254::Fr] = unsafe { transmute_copy(&evals) };

    let res: ark_bn254::G2Projective = (*SHPLONK_INSTANCE).commit_lagrange_g2(evals_ark);

    unsafe { transmute(res) }
}

pub fn shplonk_lagrange_open(evals: &[Fr], point: Fr) -> (Proof, Fr) {
    let evals_ark: &[ark_bn254::Fr] = unsafe { transmute_copy(&evals) };
    let domain = Radix2EvaluationDomain::<ark_bn254::Fr>::new(evals_ark.len()).unwrap();
    let poly = domain.ifft(evals_ark);

    let (proof, value): (ark_bn254::G1Projective, ark_bn254::Fr) =
        (*SHPLONK_INSTANCE).open(&poly, unsafe { transmute(point) });

    (Proof(unsafe { transmute(proof) }), unsafe {
        transmute(value)
    })
}

pub fn shplonk_verify(commit: Commit, proof: Proof, eval: Fr, point: Fr) -> bool {
    (*SHPLONK_INSTANCE).verify(
        unsafe { transmute(point) },
        unsafe { transmute(eval) },
        unsafe { transmute(commit.0) },
        unsafe { transmute(proof.0) },
    )
}

#[test]
fn test_commit() {
    use rand_chacha::{rand_core::SeedableRng, ChaCha12Rng};
    use snark_verifier_sdk::snark_verifier::util::arithmetic::Field;

    let mut rng = ChaCha12Rng::seed_from_u64(0);

    let k = 16;
    let n = 1 << k;
    let evals: Vec<Fr> = (0..n).map(|_| Fr::random(&mut rng)).collect();
    let point: Fr = Fr::random(&mut rng);

    let commit = shplonk_lagrange_commit(&evals);
    let (proof, value) = shplonk_lagrange_open(&evals, point);

    assert!(shplonk_verify(commit, proof, value, point));
}
