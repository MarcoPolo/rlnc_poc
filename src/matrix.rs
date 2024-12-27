use curve25519_dalek::Scalar;
/*
Eschelon is a structure that keeps both the eschelon form of a matrix and the transoformations
necessary to obtain these form. Self consistency
of the matrices is enforced as transform * coefficients = eschelon. The matrix transform consists
of products of elementary row operations, it is built sequentially as each incoming message arrives.
as such, when importing the i-th message, no elementary transformation has touched that row before,
thus the matrix before importing this message has a block structure
[ A | 0 ]
[ 0 | I ]
where A is the transformation matrix for the first i-1 messages and I is the identity
for the rest of the rows.

We could in principle work with smaller integer matrices like u64 instead of Scalar, but care
needs to be taken to prevent the integers to grow with the number of rows. Implementing something
like Bareiss' seems overkill at this stage.
*/
pub struct Eschelon {
    coefficients: Vec<Vec<Scalar>>,
    eschelon: Vec<Vec<Scalar>>,
    transform: Vec<Vec<Scalar>>,
}

impl Eschelon {
    pub fn new(size: usize) -> Self {
        let mut transform = vec![vec![Scalar::ZERO; size]; size];
        (0..size).for_each(|i| transform[i][i] = Scalar::ONE);

        Eschelon {
            coefficients: Vec::new(),
            eschelon: Vec::new(),
            transform,
        }
    }

    pub fn new_identity(size: usize) -> Self {
        let mut eschelon = vec![vec![Scalar::ZERO; size]; size];
        (0..size).for_each(|i| eschelon[i][i] = Scalar::ONE);
        let transform = eschelon.clone();
        let coefficients = eschelon.clone();

        Eschelon {
            coefficients,
            eschelon,
            transform,
        }
    }

    // is_full returns if the eschelon form is square.
    pub fn is_full(&self) -> bool {
        self.coefficients.len() == self.coefficients[0].len()
    }

    // add_row adds a row to the coefficients matrix and updates the eschelon form and the transform.
    // It returns false if the row is linearly dependent with the previous ones.
    pub fn add_row(&mut self, row: Vec<Scalar>) -> bool {
        if row.iter().all(|x| *x == Scalar::ZERO) {
            return false;
        }
        let current_size = self.coefficients.len();
        if current_size == row.len() {
            return false;
        }
        if current_size == 0 {
            self.eschelon
                .push(row.iter().map(|x| Scalar::from(*x)).collect());
            self.coefficients.push(row);
            return true;
        }
        let mut tr = self.transform[current_size].clone();
        let mut i = 0;
        let mut j: usize;
        let mut new_eschelon_row: Vec<Scalar> =
            row.iter().map(|x| Scalar::from(*x)).collect();
        while i < current_size {
            j = first_entry(&self.eschelon[i]).unwrap();
            let k = match first_entry(&new_eschelon_row) {
                Some(val) => val,
                None => return false,
            };
            if j < k {
                i += 1;
                continue;
            }
            if j > k {
                break;
            }
            let pivot = self.eschelon[i][j];
            let f = new_eschelon_row[j];
            new_eschelon_row
                .iter_mut()
                .zip(self.eschelon[i].iter())
                .for_each(|(x, y)| *x = pivot * (*x) - y * f);
            tr.iter_mut()
                .zip(self.transform[i].iter())
                .for_each(|(x, y)| *x = pivot * (*x) - y * f);
            i += 1;
        }
        if new_eschelon_row.iter().all(|x| *x == Scalar::ZERO) {
            return false;
        }
        self.eschelon.insert(i, new_eschelon_row);
        self.coefficients.push(row);
        if i < current_size {
            self.transform.remove(current_size);
            self.transform.insert(i, tr);
            return true;
        }
        self.transform[i] = tr;
        return true;
    }

    // compound_scalars performs a matrix multiplications. The node coefficients are kept as u32
    // while the chosen scalars are u8, we are under the assumption that there are less than 24 hops
    // and thus this operation will not overflow.
    pub fn compound_scalars(&self, scalars: &[u8]) -> Vec<Scalar> {
        (0..self.transform.len())
            .map(|j| {
                scalars
                    .iter()
                    .zip(self.coefficients.iter())
                    .map(|(x, coeffs)| Scalar::from(*x) * coeffs[j])
                    .sum()
            })
            .collect()
    }

    pub fn inverse(&self) -> Result<Vec<Vec<Scalar>>, String> {
        if self.coefficients.is_empty() {
            return Err("No coefficients to decode".to_string());
        }
        if self.eschelon.len() != self.coefficients[0].len() {
            return Err("The eschelon form is not square".to_string());
        }
        let mut inverse = self.transform.clone();
        for i in (0..self.eschelon.len()).rev() {
            let pivot = self.eschelon[i][i].invert();
            inverse[i].iter_mut().for_each(|x| *x = *x * pivot);
            for j in (i + 1)..self.eschelon.len() {
                let diff = self.eschelon[i][j] * pivot;
                for k in 0..self.eschelon.len() {
                    let actual_diff = inverse[j][k] * diff;
                    inverse[i][k] -= actual_diff;
                }
            }
        }
        Ok(inverse)
    }
}

fn first_entry<T: PartialEq + Default>(slice: &[T]) -> Option<usize> {
    let zero = T::default();
    slice.iter().position(|x| x != &zero)
}

#[cfg(test)]
mod tests {
    use super::*;
    use curve25519_dalek::Scalar;

    #[test]
    fn test_add_row() {
        let mut eschelon = Eschelon::new(3);
        assert_eq!(
            eschelon.add_row(vec![
                Scalar::from(0u32),
                Scalar::from(0u32),
                Scalar::from(0u32)
            ]),
            false
        );
        assert_eq!(
            eschelon.add_row(vec![
                Scalar::from(0u32),
                Scalar::from(0u32),
                Scalar::from(1u32)
            ]),
            true
        );
        assert_eq!(
            eschelon.add_row(vec![
                Scalar::from(0u32),
                Scalar::from(0u32),
                Scalar::from(1u32)
            ]),
            false
        );
        assert_eq!(
            eschelon.add_row(vec![
                Scalar::from(0u32),
                Scalar::from(1u32),
                Scalar::from(0u32)
            ]),
            true
        );
        assert_eq!(
            eschelon.add_row(vec![
                Scalar::from(0u32),
                Scalar::from(1u32),
                Scalar::from(0u32)
            ]),
            false
        );
        assert_eq!(
            eschelon.add_row(vec![
                Scalar::from(1u32),
                Scalar::from(0u32),
                Scalar::from(0u32)
            ]),
            true
        );
        assert_eq!(
            eschelon.add_row(vec![
                Scalar::from(1u32),
                Scalar::from(0u32),
                Scalar::from(0u32)
            ]),
            false
        );
        assert_eq!(
            eschelon.add_row(vec![
                Scalar::from(1u32),
                Scalar::from(1u32),
                Scalar::from(1u32)
            ]),
            false
        );
        assert_eq!(
            eschelon.add_row(vec![
                Scalar::from(0u32),
                Scalar::from(1u32),
                Scalar::from(1u32)
            ]),
            false
        );
        eschelon = Eschelon::new(3);
        assert_eq!(
            eschelon.add_row(vec![
                Scalar::from(0u32),
                Scalar::from(1u32),
                Scalar::from(0u32)
            ]),
            true
        );
        assert_eq!(
            eschelon.add_row(vec![
                Scalar::from(0u32),
                Scalar::from(2u32),
                Scalar::from(3u32)
            ]),
            true
        );
        assert_eq!(
            eschelon.add_row(vec![
                Scalar::from(5u32),
                Scalar::from(0u32),
                Scalar::from(1u32)
            ]),
            true
        );
        assert_eq!(
            eschelon.add_row(vec![
                Scalar::from(2u32),
                Scalar::from(0u32),
                Scalar::from(1u32)
            ]),
            false
        );
        eschelon = Eschelon::new(3);
        assert_eq!(
            eschelon.add_row(vec![
                Scalar::from(2u32),
                Scalar::from(1u32),
                Scalar::from(0u32)
            ]),
            true
        );
        assert_eq!(
            eschelon.add_row(vec![
                Scalar::from(3u32),
                Scalar::from(2u32),
                Scalar::from(1u32)
            ]),
            true
        );
    }

    #[test]
    fn test_inverse() {
        let mut eschelon = Eschelon::new(3);
        assert_eq!(eschelon.inverse().is_err(), true);
        eschelon.add_row(vec![
            Scalar::from(1u32),
            Scalar::from(0u32),
            Scalar::from(0u32),
        ]);
        eschelon.add_row(vec![
            Scalar::from(0u32),
            Scalar::from(1u32),
            Scalar::from(0u32),
        ]);
        eschelon.add_row(vec![
            Scalar::from(0u32),
            Scalar::from(0u32),
            Scalar::from(1u32),
        ]);
        let inverse = eschelon.inverse().unwrap();
        assert_eq!(inverse[0][0], Scalar::from(1u32));
        assert_eq!(inverse[0][1], Scalar::from(0u32));

        eschelon = Eschelon::new(2);
        assert_eq!(eschelon.inverse().is_err(), true);
        eschelon.add_row(vec![Scalar::from(2u32), Scalar::from(5u32)]);
        eschelon.add_row(vec![Scalar::from(1u32), Scalar::from(3u32)]);
        let inverse = eschelon.inverse().unwrap();
        assert_eq!(inverse[0][0], Scalar::from(3u32));
        assert_eq!(inverse[0][1], -Scalar::from(5u32));
        assert_eq!(inverse[1][0], -Scalar::from(1u32));
        assert_eq!(inverse[1][1], Scalar::from(2u32));
    }

    #[test]
    fn test_compound_scalars() {
        let eschelon = Eschelon::new(3);
        assert_eq!(
            eschelon.compound_scalars(&[1, 2, 3]),
            vec![Scalar::from(0u32), Scalar::from(0u32), Scalar::from(0u32)]
        );
        let mut eschelon = Eschelon::new(3);
        eschelon.add_row(vec![
            Scalar::from(2u32),
            Scalar::from(0u32),
            Scalar::from(0u32),
        ]);
        eschelon.add_row(vec![
            Scalar::from(0u32),
            Scalar::from(3u32),
            Scalar::from(1u32),
        ]);
        assert_eq!(
            eschelon.compound_scalars(&[3, 5]),
            vec![Scalar::from(6u32), Scalar::from(15u32), Scalar::from(5u32)]
        );
    }
}
