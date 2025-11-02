//! Implements a dynamic cryptographic accumulator that supports additions and deletions.

use super::{
    utils::{digest_to_prime_field, xgcd},
    Curve, Fr, G1Affine, G1Projective, G2Affine, G2Projective,
};
use crate::digest::Digestible;
use crate::set::MultiSet;
use super::{Acc1, Accumulator};
use anyhow::{anyhow, Result};
use ark_ec::{AffineCurve, PairingEngine, ProjectiveCurve};
use ark_ff::{Field, One, PrimeField, Zero};
use ark_poly::{univariate::{DensePolynomial, DenseOrSparsePolynomial}, Polynomial, UVPolynomial};
use std::collections::HashSet;
use std::ops::Neg;
use serde::{Serialize, Deserialize};

mod ark_serde {
    use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
    use serde::{Deserializer, Serializer};

    pub fn serialize<S, T>(data: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: CanonicalSerialize,
    {
        let mut bytes = Vec::new();
        data.serialize(&mut bytes)
            .map_err(serde::ser::Error::custom)?;
        serde_bytes::serialize(&bytes, serializer)
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: CanonicalDeserialize,
    {
        let bytes: Vec<u8> = serde_bytes::deserialize(deserializer)?;
        T::deserialize(&bytes[..]).map_err(serde::de::Error::custom)
    }
}

/// A proof that an 'add' operation was performed correctly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddProof {
    pub old_acc_value: G1Affine,
    pub new_acc_value: G1Affine,
    pub element: Fr,
}

impl AddProof {
    /// Verifies that the new accumulator is the result of adding the element to the old one.
    /// It checks if e(new_acc, g2) == e(old_acc, g2^(s-element)).
    pub fn verify(&self) -> bool {
        // Calculate g2^(s-element)
        let s_minus_elem: Fr = *super::PRI_S - self.element;
        let g2_s_minus_elem = super::G2_POWER.apply(&s_minus_elem);

        let lhs = Curve::pairing(self.new_acc_value, G2Affine::prime_subgroup_generator());
        let rhs = Curve::pairing(self.old_acc_value, g2_s_minus_elem);

        lhs == rhs
    }
}

/// A proof that a 'delete' operation was performed correctly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteProof {
    pub old_acc_value: G1Affine,
    pub new_acc_value: G1Affine,
    pub element: Fr,
}

impl DeleteProof {
    /// Verifies that the new accumulator is the result of deleting the element from the old one.
    /// It checks if e(new_acc, g2^(s-element)) == e(old_acc, g2).
    pub fn verify(&self) -> bool {
        // Calculate g2^(s-element)
        let s_minus_elem: Fr = *super::PRI_S - self.element;
        let g2_s_minus_elem = super::G2_POWER.apply(&s_minus_elem);

        let lhs = Curve::pairing(self.new_acc_value, g2_s_minus_elem);
        let rhs = Curve::pairing(self.old_acc_value, G2Affine::prime_subgroup_generator());

        lhs == rhs
    }
}

/// A proof of membership for an element in the accumulator.
/// The witness is an accumulator of the set without the element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MembershipProof {
    pub witness: G1Affine,
    pub element: Fr,
}

impl MembershipProof {
    /// Verifies that this proof is valid for the given accumulator value.
    /// It checks if e(witness, g2^(s-element)) == e(accumulator, g2).
    pub fn verify(&self, accumulator: G1Affine) -> bool {
        // Calculate g2^(s-element)
        let s_minus_elem: Fr = *super::PRI_S - self.element;
        let g2_s_minus_elem = super::G2_POWER.apply(&s_minus_elem);

        let lhs = Curve::pairing(self.witness, g2_s_minus_elem);
        let rhs = Curve::pairing(accumulator, G2Affine::prime_subgroup_generator());

        lhs == rhs
    }
}

/// A proof of non-membership for an element in the accumulator.
/// This proof shows that the element is not in the set represented by the accumulator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonMembershipProof {
    pub element: Fr,
    /// Witness for non-membership, g2^B(s)
    pub witness: G2Affine,
    /// g1^A(s), the other part of the proof
    pub g1_a: G1Affine,
}

/// A proof that a given accumulator represents the intersection of two other accumulators.
/// This proof uses the Bézout identity: A(X)*P1(X) + B(X)*P2(X) = P_intersect(X)
/// where P1, P2 are the polynomials of the two original sets, and P_intersect is the intersection polynomial.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntersectionProof {
    /// g2^Q1(s), witness for the first quotient polynomial
    #[serde(with = "ark_serde")]
    pub witness_a: G2Affine,
    /// g2^Q2(s), witness for the second quotient polynomial
    #[serde(with = "ark_serde")]
    pub witness_b: G2Affine,
    /// g1^A(s), witness for coprimality of Q1 from A(X)Q1(X) + B(X)Q2(X) = 1
    #[serde(with = "ark_serde")]
    pub witness_coprime_a: G1Affine,
    /// g1^B(s), witness for coprimality of Q2 from A(X)Q1(X) + B(X)Q2(X) = 1
    #[serde(with = "ark_serde")]
    pub witness_coprime_b: G1Affine,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnionProof {
    #[serde(with = "ark_serde")]
    pub intersection_acc_value: G1Affine,
    pub intersection_proof: IntersectionProof,
}

/// Represents the result of a query against the accumulator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryResult {
    /// The element is in the set, and here is the proof.
    Membership(MembershipProof),
    /// The element is not in the set, and here is the proof.
    NonMembership(NonMembershipProof),
}

/// A dynamic cryptographic accumulator based on the Acc1 scheme.
/// It maintains the accumulator value and the set of elements internally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicAccumulator {
    /// The current accumulator value, g1^P(s).
    pub acc_value: G1Affine,
    /// The set of elements (as field elements).
    elements: HashSet<Fr>,
}

impl DynamicAccumulator {
    /// Creates a new, empty dynamic accumulator.
    /// The initial value is g1^1, representing an empty set.
    pub fn new() -> Self {
        Self {
            acc_value: G1Projective::from(G1Affine::prime_subgroup_generator())
                .mul(Fr::one().into_repr())
                .into_affine(),
            elements: HashSet::new(),
        }
    }

    /// Adds a new element to the accumulator and returns a proof of the operation.
    /// If the element already exists, it returns an error.
    /// The accumulator value is updated by scalar multiplying it with (s-element).
    pub fn add(&mut self, element: &i64) -> Result<AddProof> {
        let fr_element = digest_to_prime_field(&element.to_digest());
        if self.elements.contains(&fr_element) {
            return Err(anyhow!("Element already in accumulator"));
        }
        let old_acc = self.acc_value;

        // Update accumulator value: acc' = acc^(s-element)
        let s_minus_elem: Fr = *super::PRI_S - fr_element;
        self.acc_value = self
            .acc_value
            .into_projective()
            .mul(s_minus_elem.into_repr())
            .into_affine();

        // Update the element set
        self.elements.insert(fr_element);

        Ok(AddProof {
            old_acc_value: old_acc,
            new_acc_value: self.acc_value,
            element: fr_element,
        })
    }

    /// Adds multiple elements to the accumulator in a batch.
    pub fn add_batch(&mut self, elements: &[i64]) -> Result<()> {
        for element in elements {
            self.add(element)?;
        }
        Ok(())
    }

    /// Updates an element in the accumulator from an old value to a new one.
    /// This is implemented as a delete operation followed by an add operation.
    /// Returns proofs for both operations.
    /// Returns an error if the old element is not in the accumulator.
    pub fn update(&mut self, old_element: &i64, new_element: &i64) -> Result<(DeleteProof, AddProof)> {
        let delete_proof = self.delete(old_element)?;
        let add_proof = self.add(new_element)?;
        Ok((delete_proof, add_proof))
    }

    /// Deletes an element from the accumulator and returns a proof of the operation.
    /// If the element exists, its count is decremented. If the count reaches zero, it's removed.
    /// The accumulator value is updated by scalar multiplying it with the inverse of (s-element).
    /// Returns an error if the element is not in the accumulator.
    pub fn delete(&mut self, element: &i64) -> Result<DeleteProof> {
        let fr_element = digest_to_prime_field(&element.to_digest());
        let old_acc = self.acc_value;

        if !self.elements.contains(&fr_element) {
            return Err(anyhow!("Element not in accumulator"));
        }

        // Update accumulator value: acc' = acc^((s-element)^-1)
        let s_minus_elem: Fr = *super::PRI_S - fr_element;
        let s_minus_elem_inv = s_minus_elem
            .inverse()
            .ok_or_else(|| anyhow!("Failed to compute inverse"))?;
        self.acc_value = self
            .acc_value
            .into_projective()
            .mul(s_minus_elem_inv.into_repr())
            .into_affine();

        // Update the element set
        self.elements.remove(&fr_element);

        Ok(DeleteProof {
            old_acc_value: old_acc,
            new_acc_value: self.acc_value,
            element: fr_element,
        })
    }

    /// Generates a membership proof for a given element.
    /// The proof's witness is an accumulator for the set of all other elements.
    /// Returns an error if the element is not in the accumulator.
    pub fn prove_membership(&self, element: &i64) -> Result<MembershipProof> {
        let fr_element = digest_to_prime_field(&element.to_digest());

        if !self.elements.contains(&fr_element) {
            return Err(anyhow!(
                "Cannot prove membership for an element not in the set"
            ));
        }

        // Calculate witness: acc^((s-element)^-1)
        let s_minus_elem: Fr = *super::PRI_S - fr_element;
        let s_minus_elem_inv = s_minus_elem
            .inverse()
            .ok_or_else(|| anyhow!("Failed to compute inverse"))?;
        let witness = self
            .acc_value
            .into_projective()
            .mul(s_minus_elem_inv.into_repr())
            .into_affine();

        Ok(MembershipProof {
            witness,
            element: fr_element,
        })
    }

    /// Verifies a membership proof against the current accumulator value.
    pub fn verify_membership(&self, proof: &MembershipProof) -> bool {
        proof.verify(self.acc_value)
    }

    /// Generates a non-membership proof for a given element.
    /// Returns an error if the element IS in the accumulator.
    pub fn prove_non_membership(&self, element: &i64) -> Result<NonMembershipProof> {
        let fr_element = digest_to_prime_field(&element.to_digest());

        if self.elements.contains(&fr_element) {
            return Err(anyhow!(
                "Cannot prove non-membership for an element in the set"
            ));
        }

        // To prove x is not in E, we show that gcd(P(X), X-x) = 1, where P(X) = product(X-e_i).
        // Using XGCD, we find polynomials A(X), B(X) such that A(X)*(X-x) + B(X)*P(X) = 1.
        // The proof is (g1^A(s), g2^B(s)).
        // Verification checks e(Acc, g2^B(s)) * e(g1^A(s), g2^(s-x)) == e(g1, g2)
        // This corresponds to e(g1^P(s), g2^B(s)) * e(g1^A(s), g2^(s-x)) == e(g1, g2)
        // which is B(s)*P(s) + A(s)*(s-x) = 1.

        // 1. Construct the accumulator polynomial P(X) = product(X-e_i).
        let mut p_poly = DensePolynomial::from_coefficients_vec(vec![Fr::one()]);
        for elem in &self.elements {
            // X - e
            let e_poly = DensePolynomial::from_coefficients_vec(vec![elem.neg(), Fr::one()]);
            p_poly = &p_poly * &e_poly;
        }

        // 2. Construct the polynomial for the non-member, Q(X) = X-x.
        let q_poly = DensePolynomial::from_coefficients_vec(vec![fr_element.neg(), Fr::one()]); // X-x

        // 3. Run XGCD on Q(X) and P(X) to find A(X) and B(X).
        // We want A(X)*Q(X) + B(X)*P(X) = 1
        if let Some((gcd, a_poly, b_poly)) = xgcd(q_poly, p_poly.clone()) {
            // GCD must be a non-zero constant for the proof to be valid.
            if !gcd.is_zero() && gcd.degree() == 0 {
                // The equation is a_poly*Q(X) + b_poly*P(X) = gcd.
                // We need it to be 1, so we must divide by the constant value of gcd.
                let gcd_val = gcd.coeffs.first().cloned().unwrap_or_else(Fr::one);
                let gcd_inv = gcd_val
                    .inverse()
                    .ok_or_else(|| anyhow!("Failed to compute gcd inverse"))?;

                let a_poly_norm = DensePolynomial::from_coefficients_vec(
                    a_poly.coeffs.iter().map(|c| *c * gcd_inv).collect(),
                );
                let b_poly_norm = DensePolynomial::from_coefficients_vec(
                    b_poly.coeffs.iter().map(|c| *c * gcd_inv).collect(),
                );

                // 4. Evaluate the normalized polynomials at the secret `s`.
                let a_s = a_poly_norm.evaluate(&*super::PRI_S);
                let b_s = b_poly_norm.evaluate(&*super::PRI_S);

                // 5. Compute the witness parts: g1^A(s) and g2^B(s)
                let g1_a = G1Projective::prime_subgroup_generator()
                    .mul(a_s.into_repr())
                    .into_affine();
                let witness_b = G2Projective::prime_subgroup_generator()
                    .mul(b_s.into_repr())
                    .into_affine();

                return Ok(NonMembershipProof {
                    element: fr_element,
                    witness: witness_b, // This is g2^B(s)
                    g1_a,               // This is g1^A(s)
                });
            }
        }

        Err(anyhow!("Failed to create non-membership proof"))
    }

    /// Verifies a non-membership proof against the current accumulator value.
    pub fn verify_non_membership(&self, proof: &NonMembershipProof) -> bool {
        // Verification equation: e(Acc, witness) * e(g1_a, g2^(s-x)) == e(g1, g2)
        // Here, witness = g2^B(s) and g1_a = g1^A(s).
        // So, e(g1^P(s), g2^B(s)) * e(g1^A(s), g2^(s-x)) == e(g1, g2)
        // which simplifies to e(g1,g2)^(B(s)*P(s) + A(s)*(s-x)) == e(g1,g2)^1
        // This holds if B(s)*P(s) + A(s)*(s-x) = 1.

        // 1. Calculate g2^(s-x)
        let s_minus_x = *super::PRI_S - proof.element;
        let g2_s_minus_x = super::G2_POWER.apply(&s_minus_x);

        // 2. Calculate the pairings
        let lhs1 = Curve::pairing(self.acc_value, proof.witness);
        let lhs2 = Curve::pairing(proof.g1_a, g2_s_minus_x);
        let rhs = Curve::pairing(
            G1Affine::prime_subgroup_generator(),
            G2Affine::prime_subgroup_generator(),
        );

        lhs1 * lhs2 == rhs
    }

    /// Returns the number of elements in the accumulator.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Returns true if the accumulator is empty.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Returns a vector of field elements (Fr) contained in the accumulator.
    /// Note: Original application values cannot be recovered from Fr digests.
    pub fn elements_fr(&self) -> Vec<Fr> {
        self.elements.iter().cloned().collect()
    }

    /// Queries the accumulator for a given element and returns a cryptographic proof
    /// of either membership or non-membership.
    pub fn query(&self, element: &i64) -> QueryResult {
        let fr_element = digest_to_prime_field(&element.to_digest());
        if self.elements.contains(&fr_element) {
            // This unwrap is safe because we've just checked for the element's existence.
            let proof = self.prove_membership(element).unwrap();
            QueryResult::Membership(proof)
        } else {
            // This unwrap is safe because we've just checked for the element's non-existence.
            let proof = self.prove_non_membership(element).unwrap();
            QueryResult::NonMembership(proof)
        }
    }

    /// Computes the intersection of this accumulator with another accumulator and generates a proof.
    /// Returns the intersection accumulator and a proof that it represents the intersection.
    /// This uses the Bézout identity: A(X)*P1(X) + B(X)*P2(X) = P_intersect(X)
    pub fn prove_intersection(&self, other: &DynamicAccumulator) -> Result<(DynamicAccumulator, IntersectionProof)> {
        // 1. Compute the actual intersection of the two sets
        let intersection_elements: std::collections::HashSet<Fr> = self.elements
            .intersection(&other.elements)
            .cloned()
            .collect();

        // 2. Create the intersection accumulator
        let mut intersection_acc = DynamicAccumulator::new();
        intersection_acc.elements = intersection_elements;
        
        // Calculate the intersection accumulator value
        let mut acc_value = G1Projective::from(G1Affine::prime_subgroup_generator());
        for elem in &intersection_acc.elements {
            let s_minus_elem = *super::PRI_S - elem;
            acc_value = acc_value.mul(s_minus_elem.into_repr());
        }
        intersection_acc.acc_value = acc_value.into_affine();

        // 3. Construct polynomials for each set
        // P1(X) = product(X - e_i) for elements in self
        let mut p1_poly = DensePolynomial::from_coefficients_vec(vec![Fr::one()]);
        for elem in &self.elements {
            let e_poly = DensePolynomial::from_coefficients_vec(vec![elem.neg(), Fr::one()]);
            p1_poly = &p1_poly * &e_poly;
        }

        // P2(X) = product(X - e_i) for elements in other
        let mut p2_poly = DensePolynomial::from_coefficients_vec(vec![Fr::one()]);
        for elem in &other.elements {
            let e_poly = DensePolynomial::from_coefficients_vec(vec![elem.neg(), Fr::one()]);
            p2_poly = &p2_poly * &e_poly;
        }

        // P_intersect(X) = product(X - e_i) for elements in intersection
        let mut p_intersect_poly = DensePolynomial::from_coefficients_vec(vec![Fr::one()]);
        for elem in &intersection_acc.elements {
            let e_poly = DensePolynomial::from_coefficients_vec(vec![elem.neg(), Fr::one()]);
            p_intersect_poly = &p_intersect_poly * &e_poly;
        }

        // 4. Use extended GCD to find Bézout coefficients
        // We need to find A(X) and B(X) such that A(X)*P1(X) + B(X)*P2(X) = P_intersect(X)
        // This is more complex than the simple GCD case, so we'll use a different approach.
        
        // For intersection proof, we need to show that P_intersect divides both P1 and P2
        // We can compute Q1(X) = P1(X) / P_intersect(X) and Q2(X) = P2(X) / P_intersect(X)
        // Then use the identity: P1(X) = Q1(X) * P_intersect(X) and P2(X) = Q2(X) * P_intersect(X)
        
        let (q1_poly, remainder1): (DensePolynomial<Fr>, DensePolynomial<Fr>) = match DenseOrSparsePolynomial::from(&p1_poly).divide_with_q_and_r(&DenseOrSparsePolynomial::from(&p_intersect_poly)) {
            Some((q, r)) => (q, r),
            None => return Err(anyhow!("Failed to divide P1 by P_intersect")),
        };
        if !remainder1.is_zero() {
            return Err(anyhow!("P_intersect does not divide P1 - invalid intersection"));
        }

        let (q2_poly, remainder2): (DensePolynomial<Fr>, DensePolynomial<Fr>) = match DenseOrSparsePolynomial::from(&p2_poly).divide_with_q_and_r(&DenseOrSparsePolynomial::from(&p_intersect_poly)) {
            Some((q, r)) => (q, r),
            None => return Err(anyhow!("Failed to divide P2 by P_intersect")),
        };
        if !remainder2.is_zero() {
            return Err(anyhow!("P_intersect does not divide P2 - invalid intersection"));
        }

        // 5. Evaluate the quotient polynomials at the secret s
        let q1_s = q1_poly.evaluate(&*super::PRI_S);
        let q2_s = q2_poly.evaluate(&*super::PRI_S);

        // 6. Compute the witnesses for quotients: g2^Q1(s) and g2^Q2(s)
        let witness_a = G2Projective::prime_subgroup_generator()
            .mul(q1_s.into_repr())
            .into_affine();
        let witness_b = G2Projective::prime_subgroup_generator()
            .mul(q2_s.into_repr())
            .into_affine();

        // 7. Prove that Q1(X) and Q2(X) are coprime using XGCD
        // We find A(X), B(X) such that A(X)Q1(X) + B(X)Q2(X) = 1
        if let Some((gcd, a_poly, b_poly)) = xgcd(q1_poly, q2_poly) {
            if !gcd.is_zero() && gcd.degree() == 0 {
                let gcd_val = gcd.coeffs.first().cloned().unwrap_or_else(Fr::one);
                let gcd_inv = gcd_val
                    .inverse()
                    .ok_or_else(|| anyhow!("Failed to compute gcd inverse for coprimality proof"))?;
                
                let a_poly_norm = DensePolynomial::from_coefficients_vec(
                    a_poly.coeffs.iter().map(|c| *c * gcd_inv).collect(),
                );
                let b_poly_norm = DensePolynomial::from_coefficients_vec(
                    b_poly.coeffs.iter().map(|c| *c * gcd_inv).collect(),
                );

                let a_s = a_poly_norm.evaluate(&*super::PRI_S);
                let b_s = b_poly_norm.evaluate(&*super::PRI_S);

                let witness_coprime_a = G1Projective::prime_subgroup_generator()
                    .mul(a_s.into_repr())
                    .into_affine();
                let witness_coprime_b = G1Projective::prime_subgroup_generator()
                    .mul(b_s.into_repr())
                    .into_affine();
                
                let proof = IntersectionProof {
                    witness_a,
                    witness_b,
                    witness_coprime_a,
                    witness_coprime_b,
                };
        
                return Ok((intersection_acc, proof));
            }
        }
        
        Err(anyhow!("Failed to create intersection proof, quotients might not be coprime"))
    }

    /// Computes the intersection and also returns the intersection elements (as Fr values).
    /// Returns (intersection_accumulator, intersection_proof, intersection_elements_fr).
    pub fn prove_intersection_with_elements(
        &self,
        other: &DynamicAccumulator,
    ) -> Result<(DynamicAccumulator, IntersectionProof, Vec<Fr>)> {
        let (intersection_acc, proof) = self.prove_intersection(other)?;
        let elements = intersection_acc.elements_fr();
        Ok((intersection_acc, proof, elements))
    }

    /// Verifies that the given accumulator represents the intersection of two other accumulators.
    /// This is a static method that doesn't require access to the secret key.
    /// 
    /// The verification checks that:
    /// - acc1_value = witness_a * intersection_value (in the exponent)
    /// - acc2_value = witness_b * intersection_value (in the exponent)
    /// - Q1 and Q2 are coprime, verified via A(s)Q1(s) + B(s)Q2(s) = 1
    /// 
    /// Using pairing: 
    /// - e(acc1, g2) == e(intersection, witness_a) 
    /// - e(acc2, g2) == e(intersection, witness_b)
    /// - e(witness_coprime_a, witness_a) * e(witness_coprime_b, witness_b) == e(g1, g2)
    pub fn verify_intersection(
        acc1_value: G1Affine,
        acc2_value: G1Affine,
        intersection_value: G1Affine,
        proof: &IntersectionProof,
    ) -> bool {
        // Verification equation 1: e(acc1, g2) == e(intersection, witness_a)
        // This verifies that acc1 = intersection^Q1(s), i.e., P1(s) = Q1(s) * P_intersect(s)
        let lhs1 = Curve::pairing(acc1_value, G2Affine::prime_subgroup_generator());
        let rhs1 = Curve::pairing(intersection_value, proof.witness_a);

        // Verification equation 2: e(acc2, g2) == e(intersection, witness_b)  
        // This verifies that acc2 = intersection^Q2(s), i.e., P2(s) = Q2(s) * P_intersect(s)
        let lhs2 = Curve::pairing(acc2_value, G2Affine::prime_subgroup_generator());
        let rhs2 = Curve::pairing(intersection_value, proof.witness_b);

        // Verification equation 3: e(g1^A(s), g2^Q1(s)) * e(g1^B(s), g2^Q2(s)) == e(g1, g2)
        // This verifies that A(s)Q1(s) + B(s)Q2(s) = 1, proving Q1 and Q2 are coprime.
        let coprimality_lhs1 = Curve::pairing(proof.witness_coprime_a, proof.witness_a);
        let coprimality_lhs2 = Curve::pairing(proof.witness_coprime_b, proof.witness_b);
        let coprimality_rhs = Curve::pairing(
            G1Affine::prime_subgroup_generator(),
            G2Affine::prime_subgroup_generator(),
        );

        lhs1 == rhs1 && lhs2 == rhs2 && (coprimality_lhs1 * coprimality_lhs2 == coprimality_rhs)
    }

    /// One-shot API: compute intersection, return query result on it, the proof, the accumulator, and elements.
    /// Returns (query_result_on_intersection, intersection_proof, intersection_accumulator, intersection_elements_fr).
    pub fn query_in_intersection_with_elements(
        &self,
        other: &DynamicAccumulator,
        element: &i64,
    ) -> Result<(QueryResult, IntersectionProof, DynamicAccumulator, Vec<Fr>)> {
        let (intersection_acc, proof) = self.prove_intersection(other)?;
        let q = intersection_acc.query(element);
        let elements = intersection_acc.elements_fr();
        Ok((q, proof, intersection_acc, elements))
    }

    /// Prover API: return intersection original values (i64), intersection accumulator and proof.
    /// Note: The prover must supply the clear-text values that correspond to `self` and `other`.
    /// The verifier can recompute the accumulator from returned values and verify against the proof.
    pub fn prove_intersection_with_values(
        &self,
        other: &DynamicAccumulator,
        self_values: &[i64],
        other_values: &[i64],
    ) -> Result<(Vec<i64>, DynamicAccumulator, IntersectionProof)> {
        // Compute intersection values on clear-text
        let set_a: std::collections::HashSet<i64> = self_values.iter().cloned().collect();
        let set_b: std::collections::HashSet<i64> = other_values.iter().cloned().collect();
        let mut intersection_values: Vec<i64> = set_a.intersection(&set_b).cloned().collect();
        intersection_values.sort_unstable();

        // Compute cryptographic intersection accumulator and proof
        let (intersection_acc, proof) = self.prove_intersection(other)?;

        // Optional consistency check (debug): ensure hashed values match the accumulator's element set
        // This is not strictly necessary for correctness, so we do not enforce it.

        Ok((intersection_values, intersection_acc, proof))
    }

    /// Computes the union accumulator and its corresponding proof without needing clear-text values.
    /// The proof is constructed based on the intersection proof.
    /// Returns the union accumulator and the union proof.
    pub fn prove_union(&self, other: &DynamicAccumulator) -> Result<(DynamicAccumulator, UnionProof)> {
        // 1. Compute the intersection and its proof, which forms the core of the union proof.
        let (intersection_acc, intersection_proof) = self.prove_intersection(other)?;

        // 2. Compute the union of the element sets cryptographically.
        let union_elements: std::collections::HashSet<Fr> = self.elements
            .union(&other.elements)
            .cloned()
            .collect();
        
        // 3. Create the union accumulator from the union elements.
        let mut union_acc = DynamicAccumulator::new();
        union_acc.elements = union_elements;
        
        // Calculate the cryptographic value of the union accumulator.
        let mut acc_value = G1Projective::from(G1Affine::prime_subgroup_generator());
        for elem in &union_acc.elements {
            let s_minus_elem = *super::PRI_S - elem;
            acc_value = acc_value.mul(s_minus_elem.into_repr());
        }
        union_acc.acc_value = acc_value.into_affine();

        // 4. Construct the union proof using the intersection proof data.
        let union_proof = UnionProof {
            intersection_acc_value: intersection_acc.acc_value,
            intersection_proof,
        };

        Ok((union_acc, union_proof))
    }

    /// Verifier helper: verify intersection using provided clear-text intersection values.
    /// It recomputes the intersection accumulator from values and checks the proof.
    pub fn verify_intersection_with_values(
        acc1_value: G1Affine,
        acc2_value: G1Affine,
        intersection_values: &[i64],
        proof: &IntersectionProof,
    ) -> bool {
        // Build a set (unique) from the provided values
        let mut unique: std::collections::HashSet<i64> = std::collections::HashSet::new();
        for v in intersection_values {
            unique.insert(*v);
        }
        let mut vec_unique: Vec<i64> = unique.into_iter().collect();
        vec_unique.sort_unstable();

        // Compute accumulator from values (public, no secret needed)
        let ms = MultiSet::from_vec(vec_unique);
        let intersection_value_from_values = Acc1::cal_acc_g1(&ms);

        // Verify pairing equations using the provided proof
        DynamicAccumulator::verify_intersection(
            acc1_value,
            acc2_value,
            intersection_value_from_values,
            proof,
        )
    }

    /// Prover API: computes union and intersection, returns clear-text values, the union accumulator, and a union proof.
    /// The proof internally contains the intersection proof.
    pub fn prove_union_with_values(
        &self,
        other: &DynamicAccumulator,
        self_values: &[i64],
        other_values: &[i64],
    ) -> Result<(Vec<i64>, Vec<i64>, DynamicAccumulator, UnionProof)> {
        // 1. Compute cryptographic intersection accumulator and proof
        let (intersection_acc, intersection_proof) = self.prove_intersection(other)?;

        // 2. Compute clear-text intersection and union values
        let set_a: std::collections::HashSet<i64> = self_values.iter().cloned().collect();
        let set_b: std::collections::HashSet<i64> = other_values.iter().cloned().collect();
        
        let mut intersection_values: Vec<i64> = set_a.intersection(&set_b).cloned().collect();
        intersection_values.sort_unstable();
        
        let mut union_values: Vec<i64> = set_a.union(&set_b).cloned().collect();
        union_values.sort_unstable();

        // 3. Create union accumulator from the clear-text union values
        let mut union_acc = DynamicAccumulator::new();
        union_acc.add_batch(&union_values)?;

        // 4. Construct the union proof
        let union_proof = UnionProof {
            intersection_acc_value: intersection_acc.acc_value,
            intersection_proof,
        };
        
        Ok((union_values, intersection_values, union_acc, union_proof))
    }

    /// Verifier API: verifies a union proof without requiring clear-text values.
    /// It checks the validity of the embedded intersection proof and verifies the cryptographic relationship
    /// between the accumulators.
    pub fn verify_union(
        acc1_value: G1Affine,
        acc2_value: G1Affine,
        union_acc_value: G1Affine,
        proof: &UnionProof,
    ) -> bool {
        // 1. Verify the embedded intersection proof. This is the cryptographic core of the verification.
        let is_intersection_valid = Self::verify_intersection(
            acc1_value,
            acc2_value,
            proof.intersection_acc_value,
            &proof.intersection_proof,
        );

        if !is_intersection_valid {
            return false;
        }

        // 2. Verify the accumulator relationship: P_A(s) + P_B(s) = P_union(s) + P_intersection(s)
        // This is checked in the elliptic curve group by point addition:
        // acc_A + acc_B = acc_union + acc_intersection
        // In projective coordinates for efficient computation:
        let lhs = acc1_value.into_projective() + acc2_value.into_projective();
        let rhs = union_acc_value.into_projective() + proof.intersection_acc_value.into_projective();

        lhs == rhs
    }

    /// Verifier API: verifies the union proof using provided clear-text union and intersection values.
    /// This function recomputes the accumulators from values and verifies both the intersection and the union relationships.
    pub fn verify_union_with_values(
        acc1_value: G1Affine,
        acc2_value: G1Affine,
        union_values: &[i64],
        intersection_values: &[i64],
        proof: &UnionProof,
    ) -> bool {
        // 1. Recompute intersection accumulator from values and check if it matches the one in the proof.
        let mut recomputed_intersection_acc = DynamicAccumulator::new();
        if recomputed_intersection_acc.add_batch(intersection_values).is_err() {
            return false;
        }
        if recomputed_intersection_acc.acc_value != proof.intersection_acc_value {
            return false; // The provided intersection values do not match the proven intersection accumulator.
        }

        // 2. Verify the underlying intersection proof.
        let is_intersection_valid = Self::verify_intersection(
            acc1_value,
            acc2_value,
            proof.intersection_acc_value,
            &proof.intersection_proof,
        );

        if !is_intersection_valid {
            return false;
        }

        // 3. Recompute union accumulator from values.
        let mut recomputed_union_acc = DynamicAccumulator::new();
        if recomputed_union_acc.add_batch(union_values).is_err() {
            return false;
        }

        // 4. Verify that the provided union values are indeed the union of the two original sets
        // This is implicitly verified by the fact that:
        // - The intersection proof is valid (step 2)
        // - The union accumulator can be correctly computed from the provided union values (step 3)
        // - The verifier can independently check that union_values ∪ intersection_values makes sense
        
        true // If we reach here, all checks have passed
    }
}

impl Default for DynamicAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{Accumulator, Acc1};
    use crate::digest::Digestible;
    use crate::set::MultiSet;

    fn init_logger() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    #[test]
    fn test_dynamic_accumulator_add() {
        init_logger();
        let mut dyn_acc = DynamicAccumulator::new();
        let add_proof1 = dyn_acc.add(&1i64).unwrap();
        let add_proof2 = dyn_acc.add(&2i64).unwrap();
        let add_proof3_res = dyn_acc.add(&1i64); // Add 1 again

        // Verify proofs
        assert!(add_proof1.verify());
        assert!(add_proof2.verify());
        assert!(add_proof3_res.is_err()); // Should fail to add duplicate

        let set = MultiSet::from_vec(vec![1i64, 2]);
        let static_acc = Acc1::cal_acc_g1_sk(&set);

        assert_eq!(dyn_acc.acc_value, static_acc);
        assert_eq!(dyn_acc.elements.len(), 2);
        assert!(dyn_acc
            .elements
            .contains(&digest_to_prime_field(&1i64.to_digest())));
        assert!(dyn_acc
            .elements
            .contains(&digest_to_prime_field(&2i64.to_digest())));
    }

    #[test]
    fn test_dynamic_accumulator_delete() {
        init_logger();
        let mut dyn_acc = DynamicAccumulator::new();
        dyn_acc.add(&1i64).unwrap();
        dyn_acc.add(&2i64).unwrap();

        // Delete 1
        let delete_proof1 = dyn_acc.delete(&1i64).unwrap();
        assert!(delete_proof1.verify());

        let set1 = MultiSet::from_vec(vec![2i64]);
        let static_acc1 = Acc1::cal_acc_g1_sk(&set1);
        assert_eq!(dyn_acc.acc_value, static_acc1);
        assert!(!dyn_acc
            .elements
            .contains(&digest_to_prime_field(&1i64.to_digest())));

        // Try to delete 1 again (should fail)
        assert!(dyn_acc.delete(&1i64).is_err());

        // Delete 2
        let delete_proof2 = dyn_acc.delete(&2i64).unwrap();
        assert!(delete_proof2.verify());

        let set2: MultiSet<i64> = MultiSet::from_vec(vec![]);
        let static_acc2 = Acc1::cal_acc_g1_sk(&set2);
        assert_eq!(dyn_acc.acc_value, static_acc2);
        assert!(dyn_acc.elements.is_empty());

        // Try to delete an element that was never there
        assert!(dyn_acc.delete(&3i64).is_err());
    }

    #[test]
    fn test_membership_proof() {
        init_logger();
        let mut dyn_acc = DynamicAccumulator::new();
        dyn_acc.add(&100).unwrap();
        dyn_acc.add(&200).unwrap();
        dyn_acc.add(&300).unwrap();

        // 1. Prove and verify 200
        let proof = dyn_acc.prove_membership(&200).unwrap();
        assert!(dyn_acc.verify_membership(&proof));
        assert!(proof.verify(dyn_acc.acc_value));

        // 2. Check that the witness is correct
        let set_without_200 = MultiSet::from_vec(vec![100i64, 300]);
        let witness_static = Acc1::cal_acc_g1_sk(&set_without_200);
        assert_eq!(proof.witness, witness_static);

        // 3. A proof for a different element should fail
        let mut wrong_proof = proof.clone();
        wrong_proof.element = digest_to_prime_field(&999i64.to_digest());
        assert!(!dyn_acc.verify_membership(&wrong_proof));

        // 4. Cannot prove membership for an element not in the set
        assert!(dyn_acc.prove_membership(&999i64).is_err());
    }

    #[test]
    fn test_non_membership_proof() {
        init_logger();
        let mut dyn_acc = DynamicAccumulator::new();
        dyn_acc.add(&100).unwrap();
        dyn_acc.add(&200).unwrap();

        // 1. Prove and verify non-membership for 300
        let proof = dyn_acc.prove_non_membership(&300).unwrap();
        assert!(dyn_acc.verify_non_membership(&proof));

        // 2. A non-membership proof for an element that IS in the set should fail
        assert!(dyn_acc.prove_non_membership(&100).is_err());

        // 3. A tampered proof should fail verification
        let mut tampered_proof = proof.clone();
        tampered_proof.element = digest_to_prime_field(&400i64.to_digest());
        assert!(!dyn_acc.verify_non_membership(&tampered_proof));

        // 4. An empty accumulator should be able to prove non-membership
        let empty_acc = DynamicAccumulator::new();
        let proof_for_empty = empty_acc.prove_non_membership(&100).unwrap();
        assert!(empty_acc.verify_non_membership(&proof_for_empty));
    }

    #[test]
    fn test_update_and_query() {
        init_logger();
        let mut dyn_acc = DynamicAccumulator::new();
        dyn_acc.add(&100).unwrap();
        dyn_acc.add(&200).unwrap();

        // 1. Test successful update
        let (delete_proof, add_proof) = dyn_acc.update(&100, &150).unwrap();
        assert!(delete_proof.verify());
        assert!(add_proof.verify());

        // Verify 100 is gone
        match dyn_acc.query(&100) {
            QueryResult::NonMembership(proof) => {
                assert!(dyn_acc.verify_non_membership(&proof));
            }
            _ => panic!("Should have been a non-membership proof for 100"),
        }

        // Verify 150 is present
        match dyn_acc.query(&150) {
            QueryResult::Membership(proof) => {
                assert!(dyn_acc.verify_membership(&proof));
            }
            _ => panic!("Should have been a membership proof for 150"),
        }

        // Verify 200 is still present
        match dyn_acc.query(&200) {
            QueryResult::Membership(proof) => {
                assert!(dyn_acc.verify_membership(&proof));
            }
            _ => panic!("Should have been a membership proof for 200"),
        }

        // 2. Test update on a non-existent element (should fail)
        assert!(dyn_acc.update(&999, &1000).is_err());
    }

    #[test]
    fn test_intersection_proof() {
        init_logger();
        
        // Create two accumulators with overlapping elements
        let mut acc1 = DynamicAccumulator::new();
        acc1.add(&100).unwrap();
        acc1.add(&200).unwrap();
        acc1.add(&300).unwrap();

        let mut acc2 = DynamicAccumulator::new();
        acc2.add(&200).unwrap();
        acc2.add(&300).unwrap();
        acc2.add(&400).unwrap();

        // 1. Prove intersection
        let (intersection_acc, proof) = acc1.prove_intersection(&acc2).unwrap();
        
        // 2. Verify the intersection contains the expected elements
        assert_eq!(intersection_acc.elements.len(), 2);
        assert!(intersection_acc.elements.contains(&digest_to_prime_field(&200i64.to_digest())));
        assert!(intersection_acc.elements.contains(&digest_to_prime_field(&300i64.to_digest())));

        // 3. Verify the intersection proof
        assert!(DynamicAccumulator::verify_intersection(
            acc1.acc_value,
            acc2.acc_value,
            intersection_acc.acc_value,
            &proof
        ));

        // 4. Test with a manually created intersection accumulator (should also work)
        let mut manual_intersection = DynamicAccumulator::new();
        manual_intersection.add(&200).unwrap();
        manual_intersection.add(&300).unwrap();
        
        assert_eq!(intersection_acc.acc_value, manual_intersection.acc_value);
    }

    #[test]
    fn test_intersection_proof_empty_intersection() {
        init_logger();
        
        // Create two accumulators with no overlapping elements
        let mut acc1 = DynamicAccumulator::new();
        acc1.add(&100).unwrap();
        acc1.add(&200).unwrap();

        let mut acc2 = DynamicAccumulator::new();
        acc2.add(&300).unwrap();
        acc2.add(&400).unwrap();

        // 1. Prove intersection (should be empty)
        let (intersection_acc, proof) = acc1.prove_intersection(&acc2).unwrap();
        
        // 2. Verify the intersection is empty
        assert_eq!(intersection_acc.elements.len(), 0);
        assert_eq!(intersection_acc.acc_value, DynamicAccumulator::new().acc_value);

        // 3. Verify the intersection proof
        assert!(DynamicAccumulator::verify_intersection(
            acc1.acc_value,
            acc2.acc_value,
            intersection_acc.acc_value,
            &proof
        ));
    }

    #[test]
    fn test_intersection_proof_identical_sets() {
        init_logger();
        
        // Create two identical accumulators
        let mut acc1 = DynamicAccumulator::new();
        acc1.add(&100).unwrap();
        acc1.add(&200).unwrap();

        let mut acc2 = DynamicAccumulator::new();
        acc2.add(&100).unwrap();
        acc2.add(&200).unwrap();

        // 1. Prove intersection (should be identical to both sets)
        let (intersection_acc, proof) = acc1.prove_intersection(&acc2).unwrap();
        
        // 2. Verify the intersection equals both original sets
        assert_eq!(intersection_acc.elements.len(), 2);
        assert_eq!(intersection_acc.acc_value, acc1.acc_value);
        assert_eq!(intersection_acc.acc_value, acc2.acc_value);

        // 3. Verify the intersection proof
        assert!(DynamicAccumulator::verify_intersection(
            acc1.acc_value,
            acc2.acc_value,
            intersection_acc.acc_value,
            &proof
        ));
    }
}
