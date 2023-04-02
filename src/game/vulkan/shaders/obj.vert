#version 450

layout (location = 0) in vec2 position;
layout (location = 1) in vec2 tex_position;
layout (location = 1) out vec2 tex_coords;
layout (location = 2) out vec4 vertex_color;
layout (location = 3) out uint textureID;
layout (location = 4) out uint material;

layout (set = 1, binding = 0) uniform Object {
    vec4 color;
    vec2 position;
    vec2 size;
    float rotation;
    uint textureID;
    uint material;
} object;

layout (set = 2, binding = 0) uniform Camera {
    vec2 position;
    float rotation;
    float zoom;
    uint mode;
} camera;

layout (push_constant) uniform PushConstant { // 128 bytes
    lowp vec2 resolution;
} pc;

mat2 rotation_matrix (float angle) { // Rotates a vertex.
    float s = sin(angle);
    float c = cos(angle);

    mat2 matrix = mat2 (
        vec2(c, -s),
        vec2(s, c)
    );
    return matrix;
}

void main() {

    tex_coords = tex_position - camera.position;// / pc.resolution) * resolutionscaler;

    // float hypo = sqrt(pow(position.x, 2) + pow(position.y, 2));
    // vec2 processedpos = vec2(
    //     cos(
    //         atan(position.y, position.x) + object.rotation
    //     ) * hypo,
    //     sin(
    //         atan(position.y, position.x) + object.rotation
    //     ) * hypo
    // ) + object.position;// * object.size;

    vec2 position = rotation_matrix(camera.rotation) * (rotation_matrix(-object.rotation) * position * object.size + object.position - camera.position);

    
    // y bound (position + pc.camera / pc.resolution) * pc.resolution.y

    // y / (x + y)

    vertex_color = object.color;
    textureID = object.textureID;
    material = object.material;

    vec2 resolutionscaler;

    switch (camera.mode) {
        case 1:
            resolutionscaler = vec2(1.0, 1.0); //stretch
            break;
        case 2:
            resolutionscaler = vec2(pc.resolution.y / (pc.resolution.x + pc.resolution.y), pc.resolution.x / (pc.resolution.x + pc.resolution.y)); //linear
            break;
        case 3:
            resolutionscaler = vec2(sin(atan(pc.resolution.y, pc.resolution.x)), cos(atan(pc.resolution.y, pc.resolution.x)))  / 0.707106781; //circle
            break;
        case 4:
            resolutionscaler = vec2(pc.resolution.y/clamp(pc.resolution.x, 0.0, pc.resolution.y), pc.resolution.x/clamp(pc.resolution.y, 0.0, pc.resolution.x)); //unfair
            break;
        case 5:
            resolutionscaler = vec2(1000 / pc.resolution.x, 1000 / pc.resolution.y); //Expand
            break;
        default:
            resolutionscaler = vec2(sin(atan(pc.resolution.y, pc.resolution.x)), cos(atan(pc.resolution.y, pc.resolution.x)))  / 0.707106781;
            break;
    }
    
    gl_Position = vec4((position * resolutionscaler), 0.0, camera.zoom);

}
