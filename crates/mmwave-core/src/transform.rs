use ndarray::array;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Serialize, Deserialize, Debug, Clone, Default)]
pub struct Transform {
    pub translation: [f32; 3], // Translation of the camera in meters, x, y, z
    pub orientation: [f32; 2], // Orientation of the camera in radians, yaw, pitch
}

impl Transform {
    pub fn apply(&self, point: [f32; 3]) -> [f32; 3] {
        let yaw = self.orientation[0];
        let pitch = self.orientation[1];

        // Yaw rotation matrix (rotate around Z-axis)
        let yaw_matrix = array![
            [yaw.cos(), -yaw.sin(), 0.0],
            [yaw.sin(), yaw.cos(), 0.0],
            [0.0, 0.0, 1.0]
        ];
        // Pitch rotation matrix (rotate around X-axis)
        let pitch_matrix = array![
            [1.0, 0.0, 0.0],
            [0.0, pitch.cos(), -pitch.sin()],
            [0.0, pitch.sin(), pitch.cos()]
        ];

        // Apply translation
        let translated_point = array![point[0], point[1], point[2]]
            + array![
                self.translation[0],
                self.translation[1],
                self.translation[2]
            ];

        // Combine rotations (first yaw, then pitch)
        let combined_rotation = pitch_matrix.dot(&yaw_matrix);

        // Apply rotation
        let rotated_point = combined_rotation.dot(&array![
            translated_point[0],
            translated_point[1],
            translated_point[2]
        ]);

        rotated_point
            .to_owned()
            .into_raw_vec()
            .try_into()
            .unwrap_or_else(|_| [0.0, 0.0, 0.0])
    }

    pub fn unapply(&self, point: [f32; 3]) -> [f32; 3] {
        let yaw = self.orientation[0];
        let pitch = self.orientation[1];

        // Inverse yaw rotation matrix (rotate around Z-axis)
        let inverse_yaw_matrix = array![
            [yaw.cos(), yaw.sin(), 0.0],
            [-yaw.sin(), yaw.cos(), 0.0],
            [0.0, 0.0, 1.0]
        ];
        // Inverse pitch rotation matrix (rotate around X-axis)
        let inverse_pitch_matrix = array![
            [1.0, 0.0, 0.0],
            [0.0, pitch.cos(), pitch.sin()],
            [0.0, -pitch.sin(), pitch.cos()]
        ];

        let rotated_point = inverse_yaw_matrix
            .dot(&inverse_pitch_matrix)
            .dot(&array![point[0], point[1], point[2]]);

        let translated_point = array![rotated_point[0], rotated_point[1], rotated_point[2]]
            - array![
                self.translation[0],
                self.translation[1],
                self.translation[2]
            ];

        translated_point
            .to_owned()
            .into_raw_vec()
            .try_into()
            .unwrap_or_else(|_| [0.0, 0.0, 0.0])
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use super::Transform;

    fn are_close(v1: [f32; 3], v2: [f32; 3], tol: f32) -> bool {
        v1.iter().zip(v2.iter()).all(|(a, b)| (a - b).abs() < tol)
    }

    #[test]
    pub fn test_apply_no_rotation_no_translation() {
        let point = [1., 2., 3.];
        let transform = Transform {
            translation: [0., 0., 0.],
            orientation: [0., 0.],
        };
        let transformed_point = transform.apply(point);
        assert!(are_close(transformed_point, point, 1e-6));
    }

    #[test]
    pub fn test_unapply_no_rotation_no_translation() {
        let point = [1., 2., 3.];
        let transform = Transform {
            translation: [0., 0., 0.],
            orientation: [0., 0.],
        };
        let transformed_point = transform.unapply(point);
        assert!(are_close(transformed_point, point, 1e-6));
    }

    #[test]
    pub fn test_apply_with_rotation() {
        let point = [1., 0., 0.];
        let transform = Transform {
            translation: [0., 0., 0.],
            orientation: [PI / 2., 0.],
        };
        let transformed_point = transform.apply(point);
        assert!(are_close(transformed_point, [0., 1., 0.], 1e-6));
    }

    #[test]
    pub fn test_unapply_with_rotation() {
        let point = [0., 1., 0.];
        let transform = Transform {
            translation: [0., 0., 0.],
            orientation: [PI / 2., 0.],
        };
        let transformed_point = transform.unapply(point);
        assert!(are_close(transformed_point, [1., 0., 0.], 1e-6));
    }

    #[test]
    pub fn test_apply_with_translation() {
        let point = [1., 0., 0.];
        let transform = Transform {
            translation: [-1., 1., 0.],
            orientation: [0., 0.],
        };
        let transformed_point = transform.apply(point);
        assert!(are_close(transformed_point, [0., 1., 0.], 1e-6));
    }

    #[test]
    pub fn test_unapply_with_translation() {
        let point = [0., 1., 0.];
        let transform = Transform {
            translation: [-1., 1., 0.],
            orientation: [0., 0.],
        };
        let transformed_point = transform.unapply(point);
        dbg!(transformed_point);
        assert!(are_close(transformed_point, [1., 0., 0.], 1e-6));
    }

    #[test]
    pub fn test_apply_unapply_complex() {
        let original_point = [3.0, -2.0, 1.0];

        let transform = Transform {
            translation: [1.0, 0.0, 0.0],
            orientation: [PI / 4., PI / 6.],
        };

        let transformed_point = transform.apply(original_point);

        let unapplied_point = transform.unapply(transformed_point);

        assert!(
            are_close(unapplied_point, original_point, 1e-6),
            "Unapplied point: {:?}, Original point: {:?}",
            unapplied_point,
            original_point
        );
    }
}
