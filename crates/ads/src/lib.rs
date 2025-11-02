pub mod merkle_tree;
pub mod patricia_trie;

/// A trait for Authenticated Data Structures (ADS).
/// This defines a common interface for different ADS implementations.
pub trait AuthenticatedDataStructure<T> {
    /// Insert data into the structure and get a commitment (e.g., root hash).
    fn commit(&mut self, data: &[T]) -> anyhow::Result<[u8; 32]>;

    /// Generate a proof of inclusion for a specific piece of data.
    fn prove(&self, element: &T) -> anyhow::Result<Vec<[u8; 32]>>;

    /// Verify a proof of inclusion against a commitment.
    fn verify(commitment: &[u8; 32], element: &T, proof: &[[u8; 32]]) -> bool;
}
