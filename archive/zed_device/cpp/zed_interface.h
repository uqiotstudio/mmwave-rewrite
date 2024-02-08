#ifdef __cplusplus
extern "C" {
#endif

#include <stddef.h>

typedef struct {
    float x, y, z;
} Point3D;

typedef struct {
    size_t num_points;
    Point3D* points;
} Body;

typedef struct {
    size_t num_bodies;
    Body* bodies;
} BodyList;

// Initialize the ZED camera
void init_zed();

// Poll for body keypoints
BodyList poll_body_keypoints();

// Clean up and release resources
void close_zed();

// Free allocated memory for body list
void free_body_list(BodyList body_list);

#ifdef __cplusplus
}
#endif
