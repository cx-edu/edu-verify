use std::{collections::BTreeMap, path::Path};

use log::debug;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use snark_verifier_sdk::{
    evm::gen_evm_proof_shplonk,
    snark_verifier::{
        halo2_base::{
            gates::{
                circuit::{builder::BaseCircuitBuilder, CircuitBuilderStage},
                GateInstructions,
            },
            AssignedValue, Context,
        },
        halo2_ecc::{bn254::FpChip, fields::FieldChip},
    },
};
use snark_verifier_sdk::{
    gen_pk,
    snark_verifier::halo2_base::{
        gates::flex_gate::MultiPhaseThreadBreakPoints,
        halo2_proofs::{
            halo2curves::bn256::{Bn256, Fr, G1Affine},
            plonk::ProvingKey,
            poly::{commitment::ParamsProver, kzg::commitment::ParamsKZG},
        },
    },
};

use crate::{
    handler::{decompse_edu_data, RANDOM},
    services::{
        poseidon::{poseidon, poseidon_chip},
        util::compress_fr,
    },
};

const MAX_N_EDU: usize = 2;
const MAX_N_EDU_MSG: usize = 2;
const MAX_N_EDU_MSG_DATA: usize = 10;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct CircuitParams {
    pub degree: u32,
    pub num_instance: usize,
    pub lookup_bits: usize,
    pub limb_bits: usize,
    pub num_limbs: usize,
}

// t段学历
// 每段学历k条信息
// 每条信息a个公开数据项, b个非公开数据项
// instance = [m_01, ..., m0k, ..., m_tk, ..., xi, M_0, ..., M_t]
fn create_edu_auth_constraint(
    param: CircuitParams,
    stage: CircuitBuilderStage,
    break_points: Option<MultiPhaseThreadBreakPoints>,
    origin_data_vec_vec: &[Vec<BTreeMap<String, Value>>],
    selected_fields_vec_vec: &[Vec<Vec<String>>],
) -> BaseCircuitBuilder<Fr> {
    let mut builder = BaseCircuitBuilder::<Fr>::from_stage(stage)
        .use_k(param.degree as usize)
        .use_lookup_bits(param.lookup_bits)
        .use_instance_columns(1);

    if let Some(break_points) = break_points {
        builder.set_break_points(break_points);
    }

    let range = builder.range_chip();
    let fp_chip = FpChip::<Fr>::new(&range, param.limb_bits, param.num_limbs);

    let ctx = builder.main(0);

    let random_constant = ctx.load_constant(*RANDOM);

    let mut instances: Vec<AssignedValue<Fr>> = Vec::new();

    let mut compress_data_vec_vec = Vec::new();
    for (origin_data_vec, selected_field_vec) in origin_data_vec_vec
        .iter()
        .zip(selected_fields_vec_vec.iter())
    {
        let mut compress_data_vec = Vec::new();
        for (origin_data, selected_fields) in origin_data_vec.iter().zip(selected_field_vec.iter())
        {
            let decompress_data = decompse_edu_data(origin_data);

            let assign_decompress_data: Vec<AssignedValue<Fr>> =
                ctx.assign_witnesses(decompress_data.clone());

            let label = origin_data
                .keys()
                .map(|key| {
                    if selected_fields.contains(key) {
                        ctx.load_witness(Fr::one())
                    } else {
                        ctx.load_witness(Fr::zero())
                    }
                })
                .collect::<Vec<_>>();

            let instance = assign_decompress_data
                .iter()
                .zip(label.iter())
                .map(|(d, l)| fp_chip.gate().mul(ctx, *d, *l))
                .collect::<Vec<_>>();

            let compress_data =
                compress_chip(ctx, &fp_chip, &assign_decompress_data, random_constant);
            instances.extend_from_slice(&instance);
            instances.push(compress_data);
            compress_data_vec.push(compress_data);
        }
        compress_data_vec_vec.push(compress_data_vec);
    }

    let xi = poseidon_chip(
        ctx,
        &compress_data_vec_vec
            .clone()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>(),
    );
    instances.push(xi);

    for compress_data_vec in compress_data_vec_vec {
        let compress_data = compress_chip(ctx, &fp_chip, &compress_data_vec, xi);
        instances.push(compress_data);
    }

    let assign_instance = &mut builder.assigned_instances;
    assign_instance[0].extend_from_slice(&instances);

    builder.calculate_params(None);

    builder
}

fn compress_chip(
    ctx: &mut Context<Fr>,
    fp_chip: &FpChip<Fr>,
    data: &[AssignedValue<Fr>],
    constant: AssignedValue<Fr>,
) -> AssignedValue<Fr> {
    let mut compress_res = ctx.load_zero();

    for d in data.iter().rev() {
        compress_res = fp_chip.gate().mul(ctx, compress_res, constant);
        compress_res = fp_chip.gate().add(ctx, compress_res, *d);
    }

    compress_res
}

pub struct CircuitProver {
    srs: ParamsKZG<Bn256>,
    pk: ProvingKey<G1Affine>,
    circuit_param: CircuitParams,
    break_points: MultiPhaseThreadBreakPoints,
}

impl CircuitProver {
    pub fn new(circuit_param: CircuitParams, srs: ParamsKZG<Bn256>) -> Self {
        let (mock_edu_vec, mock_edu_zk_label_vec) = mock_edu_data();

        let circuit = create_edu_auth_constraint(
            circuit_param,
            CircuitBuilderStage::Keygen,
            None,
            &mock_edu_vec,
            &mock_edu_zk_label_vec,
        );

        let pk = gen_pk(&srs, &circuit, None);

        let break_points = circuit.break_points();

        Self {
            srs,
            pk,
            circuit_param,
            break_points,
        }
    }

    pub fn gen_proof(
        &self,
        edu_vec: &[Vec<BTreeMap<String, Value>>],
        edu_label_vec: &[Vec<Vec<String>>],
    ) -> Vec<u8> {
        let (edu_vec, edu_label_vec) = Self::padding(edu_vec, edu_label_vec);
        let circuit = create_edu_auth_constraint(
            self.circuit_param,
            CircuitBuilderStage::Prover,
            Some(self.break_points.clone()),
            &edu_vec,
            &edu_label_vec,
        );

        let instances: Vec<Fr> = circuit.assigned_instances[0]
            .iter()
            .map(|a| a.value.evaluate())
            .collect();

        gen_evm_proof_shplonk(&self.srs, &self.pk, circuit, vec![instances])
    }

    fn padding(
        edu_vec: &[Vec<BTreeMap<String, Value>>],
        edu_label_vec: &[Vec<Vec<String>>],
    ) -> (Vec<Vec<BTreeMap<String, Value>>>, Vec<Vec<Vec<String>>>) {
        let mut edu_vec = edu_vec.to_vec();
        let mut edu_label_vec = edu_label_vec.to_vec();

        let padd_edu: BTreeMap<String, Value> = (0..MAX_N_EDU_MSG_DATA)
            .map(|i| (format!("id_{i}"), Value::String(String::from("1"))))
            .collect();
        let padd_edu_vec = vec![padd_edu.clone(); MAX_N_EDU_MSG];

        for edu_vec in edu_vec.iter_mut() {
            for edu in edu_vec.iter_mut() {
                if edu.len() < MAX_N_EDU_MSG_DATA {
                    debug!(
                        "edu 长度不足，进行填充, edu_vec 长度: {}, 需要填充: {}",
                        edu.len(),
                        MAX_N_EDU_MSG_DATA - edu.len()
                    );
                    (0..MAX_N_EDU_MSG_DATA - edu.len()).for_each(|i| {
                        edu.insert(format!("padding_{i}"), Value::String(String::from("1")));
                    });
                }
            }
            edu_vec.resize(MAX_N_EDU_MSG, padd_edu.clone());
        }
        edu_vec.resize(MAX_N_EDU, padd_edu_vec);

        for edu_label_vec in edu_label_vec.iter_mut() {
            edu_label_vec.resize(MAX_N_EDU_MSG, Vec::new());
        }
        edu_label_vec.resize(MAX_N_EDU, vec![Vec::new(); MAX_N_EDU_MSG]);

        (edu_vec, edu_label_vec)
    }
}

#[test]
fn test_edu_auth_circuit() {
    use snark_verifier_sdk::evm::evm_verify;
    use snark_verifier_sdk::evm::gen_evm_verifier_shplonk;
    use snark_verifier_sdk::snark_verifier::halo2_base::utils::fs::gen_srs;

    let circuit_param = CircuitParams {
        degree: 16,
        num_instance: 1,
        lookup_bits: 16,
        limb_bits: 88,
        num_limbs: 3,
    };
    let srs = gen_srs(circuit_param.degree);

    let (mock_edu_vec, mock_edu_label_vec) = mock_edu_data();

    let prover = CircuitProver::new(circuit_param, srs);
    let proof = prover.gen_proof(&mock_edu_vec, &mock_edu_label_vec);

    let verifier_params = prover.srs.verifier_params();

    let instances = gen_instance(&mock_edu_vec, &mock_edu_label_vec);
    let num_instances = vec![instances.len()];
    let deployment_code = gen_evm_verifier_shplonk::<BaseCircuitBuilder<Fr>>(
        verifier_params,
        prover.pk.get_vk(),
        num_instances,
        Some(Path::new("./contracts/Halo2Verifier.sol")),
    );

    evm_verify(deployment_code, vec![instances], proof);
}

fn mock_edu_data() -> (Vec<Vec<BTreeMap<String, Value>>>, Vec<Vec<Vec<String>>>) {
    let edu: BTreeMap<String, Value> = (0..MAX_N_EDU_MSG_DATA)
        .map(|i| (format!("id_{i}"), Value::String(String::from("1"))))
        .collect();
    let selected_field: Vec<String> = Vec::new();

    let edu_vec = vec![vec![edu; MAX_N_EDU_MSG]; MAX_N_EDU];
    let edu_label_vec = vec![vec![selected_field; MAX_N_EDU_MSG]; MAX_N_EDU];

    (edu_vec, edu_label_vec)
}

pub fn gen_instance(
    origin_data_vec_vec: &[Vec<BTreeMap<String, Value>>],
    selected_fields_vec_vec: &[Vec<Vec<String>>],
) -> Vec<Fr> {
    let mut instances = Vec::new();

    let mut compress_data_vec_vec = Vec::new();
    for (origin_data_vec, selected_field_vec) in origin_data_vec_vec
        .iter()
        .zip(selected_fields_vec_vec.iter())
    {
        let mut compress_data_vec = Vec::new();
        for (origin_data, selected_fields) in origin_data_vec.iter().zip(selected_field_vec.iter())
        {
            let decompress_data = decompse_edu_data(origin_data);

            let label = origin_data
                .keys()
                .map(|key| {
                    if selected_fields.contains(key) {
                        Fr::one()
                    } else {
                        Fr::zero()
                    }
                })
                .collect::<Vec<_>>();

            let instance = decompress_data
                .iter()
                .zip(label.iter())
                .map(|(d, l)| *d * *l)
                .collect::<Vec<_>>();

            let compress_data = compress_fr(&decompress_data, *RANDOM);
            instances.extend_from_slice(&instance);
            instances.push(compress_data);
            compress_data_vec.push(compress_data);
        }
        compress_data_vec_vec.push(compress_data_vec);
    }

    let xi = poseidon(
        &compress_data_vec_vec
            .clone()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>(),
    );
    instances.push(xi);

    for compress_data_vec in compress_data_vec_vec {
        let compress_data = compress_fr(&compress_data_vec, xi);
        instances.push(compress_data);
    }

    instances
}
