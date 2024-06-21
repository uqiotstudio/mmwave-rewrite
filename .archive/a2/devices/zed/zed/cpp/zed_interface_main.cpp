#include "zed_interface.h"
#include <iostream>
#include <iomanip>

int main() {
    init_zed();

    std::cout << "ZED Camera Body Tracking Example\n";
    std::cout << std::fixed << std::setprecision(3);

    int i = 0;
    while (true) {  // Poll 100 times, adjust as needed
        BodyList body_list = poll_body_keypoints();

        std::cout << "Poll " << i + 1 << ": Detected " << body_list.num_bodies << " bodies\n";

        for (size_t j = 0; j < body_list.num_bodies; ++j) {
            std::cout << "  Body " << j + 1 << ": ";
            for (size_t k = 0; k < body_list.bodies[j].num_points; ++k) {
                Point3D kp = body_list.bodies[j].points[k];
                std::cout << "(" << kp.x << ", " << kp.y << ", " << kp.z << ") ";
            }
            std::cout << "\n";
        }

        free_body_list(body_list);

        // Sleep or wait for a short period if necessary
        i++;
    }

    close_zed();
    return 0;
}
