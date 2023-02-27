#version 450

layout(location = 0) in vec2 position;
layout(location = 1) in vec2 tex_position;
layout(location = 0) out vec2 v_tex_position;
layout(location = 1) out vec4 v_color;

// layout(set = 1, binding = 0) uniform Object{
//     vec4 color;
//     vec2 position;
//     vec2 size;
//     float rotation;
// } object;

// layout (push_constant) uniform PushConstant { // 128 bytes
//     lowp vec2 resolution;
//     vec2 camera;
// } pc;

void main() {
    // vec2 position = position * object.size;

    // float hypo = sqrt(pow(position.x, 2) + pow(position.y, 2));
    // vec2 processedpos = vec2(
    //     cos(
    //         atan(position.y, position.x) + object.rotation
    //     ) * hypo,
    //     sin(
    //         atan(position.y, position.x) + object.rotation
    //     ) * hypo
    // ) + object.position;// * object.size;

    // vec2 resolutionscaler = vec2(sin(atan(pc.resolution.y, pc.resolution.x)), cos(atan(pc.resolution.y, pc.resolution.x)))  / (sqrt(2) / 2);
    
    
    
    v_tex_position = tex_position;
    v_color = vec4(1.0, 1.0, 1.0, 1.0);//object.color;
    
    gl_Position = vec4(position, 0.0, 1.0);//(processedpos - pc.camera / pc.resolution) * resolutionscaler, 0.0, 1.0);
    //gl_Position = vec4(position, 0.0, 1.0);
}