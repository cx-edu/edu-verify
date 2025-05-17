use ark_ec::pairing::Pairing;
use ark_ec::CurveGroup;
use ark_ff::Field;
use ark_poly::{EvaluationDomain, GeneralEvaluationDomain};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use std::ops::Mul;

pub struct Shplonk<E: Pairing> {
    pub g1: E::G1,
    pub g2: E::G2,
    pub g2_tau: E::G2,
    pub degree: usize,
    pub crs_g1_aff: Vec<E::G1Affine>,
    pub crs_lagrange_g1_aff: Vec<E::G1Affine>,
    pub crs_g2_aff: Vec<E::G2Affine>,
    pub crs_lagrange_g2_aff: Vec<E::G2Affine>,
}

impl<E: Pairing> Shplonk<E> {
    pub fn new(g1: E::G1, g2: E::G2, degree: usize) -> Self {
        Self {
            g1,
            g2,
            degree,
            g2_tau: E::G2::default(),
            crs_g1_aff: vec![],
            crs_lagrange_g1_aff: vec![],
            crs_g2_aff: vec![],
            crs_lagrange_g2_aff: vec![],
        }
    }

    pub fn setup(&mut self, secret: E::ScalarField) {
        let crs_g1: Vec<_> = (0..self.degree)
            .into_par_iter()
            .map(|i| self.g1.mul(secret.pow([i as u64])))
            .collect();
        let crs_g2: Vec<_> = (0..self.degree)
            .into_par_iter()
            .map(|i| self.g2.mul(secret.pow([i as u64])))
            .collect();

        self.g2_tau = self.g2.mul(secret);
        self.crs_g1_aff = E::G1::normalize_batch(&crs_g1);
        self.crs_lagrange_g1_aff = g1_ifft::<E>(&crs_g1);

        self.crs_g2_aff = E::G2::normalize_batch(&crs_g2);
        self.crs_lagrange_g2_aff = g2_ifft::<E>(&crs_g2);
    }

    pub fn commit_g1(&self, poly: &[E::ScalarField]) -> E::G1 {
        assert!(poly.len() <= self.degree);
        ark_ec::VariableBaseMSM::msm(&self.crs_g1_aff[..poly.len()], poly).unwrap()
    }

    pub fn commit_lagrange_g1(&self, evals: &[E::ScalarField]) -> E::G1 {
        assert!(evals.len() <= self.degree);
        ark_ec::VariableBaseMSM::msm(&self.crs_lagrange_g1_aff[..evals.len()], evals).unwrap()
    }

    pub fn commit_lagrange_g2(&self, evals: &[E::ScalarField]) -> E::G2 {
        assert!(evals.len() <= self.degree);
        ark_ec::VariableBaseMSM::msm(&self.crs_lagrange_g2_aff[..evals.len()], evals).unwrap()
    }

    pub fn open(&self, poly: &[E::ScalarField], point: E::ScalarField) -> (E::G1, E::ScalarField) {
        // evaluate the polynomial at point
        let value = evaluate(poly, point);

        // initialize denominator
        let denominator = [-point, E::ScalarField::ONE];

        // initialize numerator
        let first = poly[0] - value;
        let rest = &poly[1..];
        let temp: Vec<E::ScalarField> =
            std::iter::once(first).chain(rest.iter().cloned()).collect();
        let numerator: &[E::ScalarField] = &temp;

        // get quotient by dividing numerator by denominator
        let quotient = div(numerator, &denominator).unwrap();

        // calculate pi as proof (quotient multiplied by CRS)
        (self.commit_g1(&quotient), value)
    }

    pub fn verify(
        &self,
        point: E::ScalarField,
        value: E::ScalarField,
        commitment: E::G1,
        pi: E::G1,
    ) -> bool {
        let lhs = E::pairing(pi, self.g2_tau - self.g2.mul(point));
        let rhs = E::pairing(commitment - self.g1.mul(value), self.g2);
        lhs == rhs
    }
}

fn g1_ifft<E: Pairing>(g1: &[E::G1]) -> Vec<E::G1Affine> {
    let degree = g1.len();

    let ifft_result: Vec<_> = GeneralEvaluationDomain::<E::ScalarField>::new(degree)
        .unwrap()
        .ifft(g1)
        .par_iter()
        .map(|p| p.into_affine())
        .collect();

    ifft_result
}

fn g2_ifft<E: Pairing>(g2: &[E::G2]) -> Vec<E::G2Affine> {
    let degree = g2.len();

    let ifft_result: Vec<_> = GeneralEvaluationDomain::<E::ScalarField>::new(degree)
        .unwrap()
        .ifft(g2)
        .par_iter()
        .map(|p| p.into_affine())
        .collect();

    ifft_result
}

// helper function for polynomial division
pub fn div<E: Field>(p1: &[E], p2: &[E]) -> Result<Vec<E>, &'static str> {
    if p2.is_empty() || p2.iter().all(|&x| x == E::ZERO) {
        return Err("Cannot divide by zero polynomial");
    }

    if p1.len() < p2.len() {
        return Ok(vec![E::ZERO]);
    }

    let mut quotient = vec![E::ZERO; p1.len() - p2.len() + 1];
    let mut remainder: Vec<E> = p1.to_vec();

    while remainder.len() >= p2.len() {
        let coeff = *remainder.last().unwrap() / *p2.last().unwrap();
        let pos = remainder.len() - p2.len();

        quotient[pos] = coeff;

        for (i, &factor) in p2.iter().enumerate() {
            remainder[pos + i] -= factor * coeff;
        }

        while let Some(true) = remainder.last().map(|x| *x == E::ZERO) {
            remainder.pop();
        }
    }

    Ok(quotient)
}

// helper function to evaluate polynomial at a point
fn evaluate<E: Field>(poly: &[E], point: E) -> E {
    let mut value = E::ZERO;

    for i in 0..poly.len() {
        value += poly[i] * point.pow([i as u64]);
    }

    value
}
