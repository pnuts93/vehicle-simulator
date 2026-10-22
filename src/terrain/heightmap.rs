//! A regular grid of elevation samples. Pure data: no Bevy ECS, no file formats.
//!
//! Grid axes follow the world: columns run east (+X) and rows run south (+Z), so row 0 is the
//! northern edge and north is -Z, matching the convention that forward is -Z. That is also how
//! raster formats such as GeoTIFF store their rows.

use std::fmt;

use bevy::math::Vec2;

#[derive(Clone, Debug, PartialEq)]
pub struct Heightmap {
    cols: usize,
    rows: usize,
    /// Metres between neighbouring samples along X (columns) and Z (rows).
    cell_size: Vec2,
    /// Row-major, `rows * cols` elevations in metres.
    elevations: Vec<f32>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum HeightmapError {
    /// Fewer than two samples along an axis: no surface can be formed.
    TooSmall {
        cols: usize,
        rows: usize,
    },
    /// The sample count does not match the stated dimensions.
    LengthMismatch {
        expected: usize,
        actual: usize,
    },
    NonFiniteElevation,
    InvalidCellSize,
}

impl fmt::Display for HeightmapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooSmall { cols, rows } => {
                write!(
                    f,
                    "heightmap of {cols}x{rows} samples is too small, need at least 2x2"
                )
            }
            Self::LengthMismatch { expected, actual } => {
                write!(f, "expected {expected} elevation samples, got {actual}")
            }
            Self::NonFiniteElevation => write!(f, "elevations must be finite"),
            Self::InvalidCellSize => write!(f, "cell size must be positive and finite"),
        }
    }
}

impl std::error::Error for HeightmapError {}

impl Heightmap {
    pub fn new(
        cols: usize,
        rows: usize,
        cell_size: Vec2,
        elevations: Vec<f32>,
    ) -> Result<Self, HeightmapError> {
        if cols < 2 || rows < 2 {
            return Err(HeightmapError::TooSmall { cols, rows });
        }
        if elevations.len() != cols * rows {
            return Err(HeightmapError::LengthMismatch {
                expected: cols * rows,
                actual: elevations.len(),
            });
        }
        if !(cell_size.x > 0. && cell_size.y > 0. && cell_size.is_finite()) {
            return Err(HeightmapError::InvalidCellSize);
        }
        if elevations.iter().any(|e| !e.is_finite()) {
            return Err(HeightmapError::NonFiniteElevation);
        }
        Ok(Self {
            cols,
            rows,
            cell_size,
            elevations,
        })
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cell_size(&self) -> Vec2 {
        self.cell_size
    }

    /// Row-major elevations, `rows * cols` of them.
    pub fn elevations(&self) -> &[f32] {
        &self.elevations
    }

    /// Elevation of the sample at `(col, row)`.
    pub fn at(&self, col: usize, row: usize) -> f32 {
        self.elevations[row * self.cols + col]
    }

    /// Like [`at`](Self::at) but out-of-range indices are clamped to the edge.
    pub fn at_clamped(&self, col: isize, row: isize) -> f32 {
        let col = col.clamp(0, self.cols as isize - 1) as usize;
        let row = row.clamp(0, self.rows as isize - 1) as usize;
        self.at(col, row)
    }

    /// Ground extent in metres between the first and last sample (X, Z).
    pub fn extent(&self) -> Vec2 {
        Vec2::new(
            (self.cols - 1) as f32 * self.cell_size.x,
            (self.rows - 1) as f32 * self.cell_size.y,
        )
    }

    /// Position of sample `(col, row)` in metres relative to the centre of the grid.
    pub fn local_xz(&self, col: usize, row: usize) -> Vec2 {
        Vec2::new(
            (col as f32 - (self.cols - 1) as f32 / 2.) * self.cell_size.x,
            (row as f32 - (self.rows - 1) as f32 / 2.) * self.cell_size.y,
        )
    }

    /// Lowest and highest elevation.
    pub fn range(&self) -> (f32, f32) {
        self.elevations
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), &e| (lo.min(e), hi.max(e)))
    }

    /// Elevation under the grid centre, interpolated if the centre falls between samples.
    pub fn center_elevation(&self) -> f32 {
        self.elevation_at(Vec2::ZERO)
    }

    /// Bilinearly interpolated elevation at a position in metres relative to the grid centre.
    /// Positions outside the grid are clamped to its edge.
    pub fn elevation_at(&self, local_xz: Vec2) -> f32 {
        let col = local_xz.x / self.cell_size.x + (self.cols - 1) as f32 / 2.;
        let row = local_xz.y / self.cell_size.y + (self.rows - 1) as f32 / 2.;
        let col = col.clamp(0., (self.cols - 1) as f32);
        let row = row.clamp(0., (self.rows - 1) as f32);
        let (c0, r0) = (col.floor() as usize, row.floor() as usize);
        let (c1, r1) = ((c0 + 1).min(self.cols - 1), (r0 + 1).min(self.rows - 1));
        let (tx, tz) = (col - c0 as f32, row - r0 as f32);
        let north = self.at(c0, r0) * (1. - tx) + self.at(c1, r0) * tx;
        let south = self.at(c0, r1) * (1. - tx) + self.at(c1, r1) * tx;
        north * (1. - tz) + south * tz
    }

    /// Surface normal at sample `(col, row)`, from central differences.
    pub fn normal(&self, col: usize, row: usize) -> [f32; 3] {
        let (c, r) = (col as isize, row as isize);
        let dx = (self.at_clamped(c + 1, r) - self.at_clamped(c - 1, r)) / (2. * self.cell_size.x);
        let dz = (self.at_clamped(c, r + 1) - self.at_clamped(c, r - 1)) / (2. * self.cell_size.y);
        let len = (dx * dx + 1. + dz * dz).sqrt();
        [-dx / len, 1. / len, -dz / len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp() -> Heightmap {
        // Elevation equals the column index, so it rises towards east.
        let elevations = (0..3).flat_map(|_| [0., 1., 2.]).collect();
        Heightmap::new(3, 3, Vec2::new(10., 10.), elevations).unwrap()
    }

    #[test]
    fn rejects_malformed_input() {
        let cell = Vec2::ONE;
        assert_eq!(
            Heightmap::new(1, 5, cell, vec![0.; 5]),
            Err(HeightmapError::TooSmall { cols: 1, rows: 5 })
        );
        assert_eq!(
            Heightmap::new(2, 2, cell, vec![0.; 3]),
            Err(HeightmapError::LengthMismatch {
                expected: 4,
                actual: 3
            })
        );
        assert_eq!(
            Heightmap::new(2, 2, cell, vec![0., 0., f32::NAN, 0.]),
            Err(HeightmapError::NonFiniteElevation)
        );
        assert_eq!(
            Heightmap::new(2, 2, Vec2::new(0., 1.), vec![0.; 4]),
            Err(HeightmapError::InvalidCellSize)
        );
    }

    #[test]
    fn interpolates_between_samples() {
        let map = ramp();
        assert_eq!(map.extent(), Vec2::new(20., 20.));
        assert_eq!(map.center_elevation(), 1.);
        assert_eq!(map.elevation_at(Vec2::new(5., 3.)), 1.5);
        assert_eq!(map.elevation_at(Vec2::new(-500., 0.)), 0.);
        assert_eq!(map.elevation_at(Vec2::new(500., 0.)), 2.);
    }

    #[test]
    fn normal_leans_away_from_the_uphill_side() {
        let n = ramp().normal(1, 1);
        assert!(n[0] < 0. && n[1] > 0. && n[2].abs() < 1e-6);
    }
}
