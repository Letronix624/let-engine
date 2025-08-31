#version 460
layout (location = 0) out vec4 f_color;
layout (location = 1) in vec2 tex_coords;
layout (location = 2) in vec4 color;
layout (location = 3) flat in uint layer;

layout (set = 0, binding = 0) uniform sampler2DArray tex;

void main() {
    f_color = texture(tex, vec3(tex_coords * 0.5 + 0.5, layer)) * color;
}

