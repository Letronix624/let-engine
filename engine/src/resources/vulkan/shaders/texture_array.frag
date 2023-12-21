#version 450
layout (location = 0) out vec4 f_color;
layout (location = 1) in vec2 tex_coords;
layout (set = 0, binding = 1) uniform Object {
	vec4 color;
	uint layer;
} object;

layout (set = 1, binding = 0) uniform sampler2DArray tex;

void main() {
    f_color = texture(tex, vec3(tex_coords * 0.5 + 0.5, object.layer)) * object.color;
}

