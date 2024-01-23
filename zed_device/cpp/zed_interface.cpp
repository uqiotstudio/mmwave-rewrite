#include "zed_interface.h"
#include <sl/Camera.hpp>
#include <vector>

using namespace sl;
using namespace std;

Camera zed;
BodyTrackingParameters detection_parameters;
BodyTrackingRuntimeParameters detection_parameters_rt;

void init_zed() {
    // Initialize ZED Camera
    InitParameters init_parameters;
    init_parameters.camera_resolution = RESOLUTION::HD720;
    init_parameters.depth_mode = DEPTH_MODE::PERFORMANCE;
    init_parameters.coordinate_units = UNIT::METER;
    init_parameters.sdk_verbose = true;

    ERROR_CODE returned_state = zed.open(init_parameters);
    if (returned_state != ERROR_CODE::SUCCESS) {
        cout << "Error " << returned_state << ", ZED Camera Initialization Failed.\n";
        exit(EXIT_FAILURE);
    }

    // Setup Body Tracking
    detection_parameters.detection_model = BODY_TRACKING_MODEL::HUMAN_BODY_MEDIUM;
    detection_parameters.body_format = BODY_FORMAT::BODY_38;
    detection_parameters.image_sync = true;
    detection_parameters.enable_tracking = true;
    detection_parameters.enable_body_fitting = true;

    if (detection_parameters.enable_tracking) {
        zed.enablePositionalTracking();
    }

    returned_state = zed.enableBodyTracking(detection_parameters);
    if (returned_state != ERROR_CODE::SUCCESS) {
        cout << "Error " << returned_state << ", Body Tracking Initialization Failed.\n";
        zed.close();
        exit(EXIT_FAILURE);
    }

    // Set runtime parameters
    detection_parameters_rt.detection_confidence_threshold = 40;
}

BodyList poll_body_keypoints() {
    BodyList body_list;
    body_list.num_bodies = 0;
    body_list.bodies = nullptr;

    if (zed.grab() == ERROR_CODE::SUCCESS) {
        Bodies bodies;
        zed.retrieveBodies(bodies, detection_parameters_rt);

        if (bodies.is_new) {
            size_t num_bodies = bodies.body_list.size();
            body_list.num_bodies = num_bodies;
            body_list.bodies = new Body[num_bodies];

            for (size_t i = 0; i < num_bodies; i++) {
                auto& current_body = bodies.body_list[i];
                size_t num_keypoints = current_body.keypoint.size();
                body_list.bodies[i].num_points = num_keypoints;
                body_list.bodies[i].points = new Point3D[num_keypoints];

                for (size_t j = 0; j < num_keypoints; j++) {
                    body_list.bodies[i].points[j].x = current_body.keypoint[j].x;
                    body_list.bodies[i].points[j].y = current_body.keypoint[j].y;
                    body_list.bodies[i].points[j].z = current_body.keypoint[j].z;
                }
            }
        }
    }

    return body_list;
}

void close_zed() {
    zed.close();
}

void free_body_list(BodyList body_list) {
    for (size_t i = 0; i < body_list.num_bodies; i++) {
        delete[] body_list.bodies[i].points;
    }
    delete[] body_list.bodies;
}

