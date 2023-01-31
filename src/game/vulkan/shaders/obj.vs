#version 450

layout (location = 0) in vec2 position;
layout (location = 1) out vec2 tex_coords;
layout (location = 2) out vec4 vertex_color;
layout (location = 3) out uint textureID;

layout (set = 1, binding = 0) uniform Object {
    vec4 color;
    vec2 position;
    vec2 size;
    float rotation;
    uint textureID;
} object;

layout (push_constant) uniform PushConstant { // 128 bytes
    lowp vec2 resolution;
    vec2 camera;
} pc;



vec4 orange = vec4(1.00, 0.50, 0.00, 1.0);
vec4 lavender = vec4(0.53, 0.4, 0.8, 1.0);
vec4 skyblue = vec4(0.10, 0.40, 1.00, 1.0);
vec4 maya = vec4(0.5, 0.75, 1.0, 1.0);


vec4 colors[] = vec4[](
    lavender, // top left
    orange, // bottom left
    lavender, // top right
    // vertex 2
    orange, // bottom left
    orange, // bottom right
    lavender, // top right
    // upper row
    skyblue, // top left
    lavender, // bottom left
    skyblue, // top right
    // vertex 2
    lavender, // bottom left
    lavender, // bottom right
    skyblue // top right
);
void main() {

    tex_coords = position - pc.camera;// / pc.resolution) * resolutionscaler;
    vec2 position = position * object.size;

    float hypo = sqrt(pow(position.x, 2) + pow(position.y, 2));
    vec2 processedpos = vec2(
        cos(
            atan(position.y, position.x) + object.rotation
        ) * hypo,
        sin(
            atan(position.y, position.x) + object.rotation
        ) * hypo
    ) + object.position;// * object.size;

    
    // y bound (position + pc.camera / pc.resolution) * pc.resolution.y

    // y / (x + y)

    vertex_color = object.color;
    textureID = object.textureID;

    //vec2 resolutionscaler = vec2(pc.resolution.y / (pc.resolution.x + pc.resolution.y), pc.resolution.x / (pc.resolution.x + pc.resolution.y));
    vec2 resolutionscaler = vec2(sin(atan(pc.resolution.y, pc.resolution.x)), cos(atan(pc.resolution.y, pc.resolution.x)))  / (sqrt(2) / 2);

    
    gl_Position = vec4((processedpos - pc.camera / pc.resolution) * resolutionscaler, 0.0, 1.0);

    

    
}
