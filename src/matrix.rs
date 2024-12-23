/*
Eschelon is a structure that keeps both the eschelon form of a matrix and the transoformations
necessary to obtain these form. The determinant of the transform matrix is kept as a separate field.
This is the only element that needs to be inverted in the Ristretto scalar field. Self consistency
of the matrices is enforced as transform * coefficients = eschelon.
*/
pub struct Eschelon {
    coefficients: Vec<Vec<u32>>,
    eschelon: Vec<Vec<u64>>,
    transform: Vec<Vec<u64>>,
    determinant: u64,
}

impl Eschelon {
    pub fn new(size: usize) -> Self {
        let mut transform = vec![vec![0; size]; size];
        (0..size).for_each(|i| transform[i][i] = 1);

        Eschelon {
            coefficients: Vec::new(),
            eschelon: vec![vec![0; size]; size],
            transform,
            determinant: 1,
        }
    }

    pub fn new_identity(size: usize) -> Self {
        let mut eschelon = vec![vec![0; size]; size];
        (0..size).for_each(|i| eschelon[i][i] = 1);
        let transform = eschelon.clone();
        let mut coefficients = vec![vec![0; size]; size];
        (0..size).for_each(|i| coefficients[i][i] = 1);

        Eschelon {
            coefficients,
            eschelon,
            transform,
            determinant: 1,
        }
    }

    pub fn add_row(&mut self, row: Vec<u32>) {
        self.coefficients.push(row);
    }

    // compound_scalars performs a matrix multiplications. The node coefficients are kept as u32
    // while the chosen scalars are u8, we are under the assumption that there are less than 24 hops
    // and thus this operation will not overflow.
    pub fn compound_scalars(&self, scalars: &[u8]) -> Vec<u32> {
        (0..scalars.len())
            .map(|j| {
                scalars
                    .iter()
                    .zip(self.coefficients.iter())
                    .map(|(x, coeffs)| *x as u32 * coeffs[j])
                    .sum()
            })
            .collect()
    }
}
