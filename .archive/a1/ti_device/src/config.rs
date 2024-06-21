use crate::radar::AwrDescriptor;
use serde;

#[derive(PartialEq, Eq, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RadarConfiguration {
    pub descriptors: Vec<AwrDescriptor>,
}

#[cfg(test)]
mod tests {
    use crate::radar::{Model, Transform};

    use super::*;

    #[test]
    fn test_radar_configuration_serialize() {
        let radar_descriptor = AwrDescriptor {
            serial: "123456".to_string(),
            model: Model::AWR1843Boost,
            config: "/path/to/config".to_string(),
            transform: Transform {},
        };

        let radar_configuration = RadarConfiguration {
            descriptors: vec![radar_descriptor],
        };

        let serialized = serde_json::to_string(&radar_configuration).unwrap();

        let deserialized: RadarConfiguration = serde_json::from_str(&serialized).unwrap();

        assert_eq!(radar_configuration, deserialized);
    }

    #[test]
    fn test_radar_descriptor_deserialize() {
        let json_data = r#"
        {
            "serial": "ABC123",
            "model": "AWR1843Boost",
            "config": "/path/to/another/config",
            "transform": {}
        }
        "#;

        let expected_radar_descriptor = AwrDescriptor {
            serial: "ABC123".to_string(),
            model: Model::AWR1843Boost,
            config: "/path/to/another/config".to_string(),
            transform: Transform {},
        };

        let deserialized: AwrDescriptor = serde_json::from_str(json_data).unwrap();

        assert_eq!(deserialized, expected_radar_descriptor);
    }

    #[test]
    fn test_radar_configuration_deserialization() {
        let json_data = r#"
        {
            "descriptors": [
                {
                    "serial": "123456",
                    "model": "AWR1843Boost",
                    "config": "/path/to/config",
                    "transform": {}
                },
                {
                    "serial": "789012",
                    "model": "AWR1843AOP",
                    "config": "/path/to/another/config",
                    "transform": {}
                }
            ]
        }
    "#;

        let expected_radar_configuration = RadarConfiguration {
            descriptors: vec![
                AwrDescriptor {
                    serial: "123456".to_string(),
                    model: Model::AWR1843Boost,
                    config: "/path/to/config".to_string(),
                    transform: Transform {},
                },
                AwrDescriptor {
                    serial: "789012".to_string(),
                    model: Model::AWR1843AOP,
                    config: "/path/to/another/config".to_string(),
                    transform: Transform {},
                },
            ],
        };

        let deserialized: RadarConfiguration = serde_json::from_str(json_data).unwrap();

        assert_eq!(deserialized, expected_radar_configuration);
    }
}
