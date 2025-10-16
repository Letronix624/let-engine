#version 450
layout (location = 1) in vec3 v_normal;
layout (location = 2) in vec2 tex_position;
layout (set = 1, binding = 0) uniform Object {
	vec4 color;
} object;

layout (location = 0) out vec4 f_color;

layout (set = 2, binding = 0) uniform sampler2D tex;

const vec3 LIGHT = vec3(1.0, 1.0, 1.0);

void main() {
	float brightness = dot(normalize(v_normal), normalize(LIGHT));

	vec4 tex = texture(tex, tex_position);// * 0.5 + 0.5);

    f_color = mix(tex, tex * object.color, brightness);
}

