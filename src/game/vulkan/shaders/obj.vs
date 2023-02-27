#version 450

layout (location = 0) in vec2 position;
layout (location = 1) in vec2 tex_position;
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


void main() {

    tex_coords = tex_position - pc.camera;// / pc.resolution) * resolutionscaler;
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

    //vec2 resolutionscaler = vec2(pc.resolution.y / (pc.resolution.x + pc.resolution.y), pc.resolution.x / (pc.resolution.x + pc.resolution.y)); //cube
    //vec2 resolutionscaler = vec2(sin(atan(pc.resolution.y, pc.resolution.x)), cos(atan(pc.resolution.y, pc.resolution.x)))  / 0.707106781; //sphere
    vec2 resolutionscaler = vec2(pc.resolution.y/clamp(pc.resolution.x, 0.0, pc.resolution.y), pc.resolution.x/clamp(pc.resolution.y, 0.0, pc.resolution.x)); //unfair

    
    gl_Position = vec4((processedpos - pc.camera / pc.resolution) * resolutionscaler, 0.0, 1.0);

    

    
}
