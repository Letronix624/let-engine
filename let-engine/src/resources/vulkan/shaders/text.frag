#version 450
layout (location = 0) out vec4 f_color;
layout (location = 1) in vec2 tex_coords;

layout (set = 0, binding = 1) uniform Object {
	vec4 color;
	uint layer;
} object;

layout (set = 1, binding = 0) uniform sampler2D tex;

void main() {
    f_color = vec4(object.color.rgb, texture(tex, tex_coords).r * object.color.a);
}

