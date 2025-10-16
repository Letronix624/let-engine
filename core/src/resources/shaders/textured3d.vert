#version 450

layout (location = 0) in vec3 position;
layout (location = 1) in vec2 tex_position;
layout (location = 2) in vec3 normal;

layout (location = 1) out vec3 v_normal;
layout (location = 2) out vec2 frag_position;

layout (set = 0, binding = 0) readonly uniform ViewProj {
	mat4 view;
	mat4 proj;
} view_proj;

layout (push_constant) uniform pc {
	mat4 model;
} model;

void main() {
	mat4 modelview = view_proj.view * model.model;

	frag_position = tex_position;
	v_normal = transpose(inverse(mat3(modelview))) * normal;

    gl_Position = view_proj.proj * modelview * vec4(position, 1.0);

}
