use pse_poseidon::Poseidon;
use snark_verifier_sdk::snark_verifier::halo2_base::{
    gates::GateChip, halo2_proofs::halo2curves::bn256::Fr, poseidon::hasher::PoseidonSponge,
    AssignedValue, Context,
};

const T: usize = 3;
const RATE: usize = 2;
const R_F: usize = 8;
const R_P: usize = 57;

pub fn poseidon_chip(
    ctx: &mut Context<Fr>,
    absorptions: &[AssignedValue<Fr>],
) -> AssignedValue<Fr> {
    let gate: GateChip<Fr> = GateChip::default();

    let mut circuit_sponge = PoseidonSponge::<Fr, T, RATE>::new::<R_F, R_P, 0>(ctx);

    circuit_sponge.update(absorptions);

    circuit_sponge.squeeze(ctx, &gate)
}

pub fn poseidon(absorptions: &[Fr]) -> Fr {
    let mut native_sponge = Poseidon::<Fr, T, RATE>::new(R_F, R_P);

    native_sponge.update(absorptions);

    native_sponge.squeeze()
}

#[test]
fn test_poseidon_hash() {
    use rand_chacha::rand_core::SeedableRng;
    use rand_chacha::ChaCha20Rng;
    use snark_verifier_sdk::snark_verifier::halo2_base::gates::flex_gate::threads::SinglePhaseCoreManager;
    use snark_verifier_sdk::snark_verifier::halo2_base::halo2_proofs::arithmetic::Field;

    let mut rng = ChaCha20Rng::seed_from_u64(0);
    let n = 256;

    let mut pool = SinglePhaseCoreManager::new(true, Default::default());
    let ctx = pool.main();

    let data: Vec<Fr> = (0..n).map(|_| Fr::random(&mut rng)).collect();

    let assign_data = ctx.assign_witnesses(data.clone());
    let circuit_res = poseidon_chip(ctx, &assign_data).value.evaluate();

    let native_res = poseidon(&data);

    assert_eq!(native_res, circuit_res);
}
