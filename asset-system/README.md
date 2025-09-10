# Asset system

Asset system for let-engine that allows you to pack your resources from a asset path
to be output next to your binary.

At runtime those resources can easily be accessed using a function that loads and caches
those resources.

Using a config located at the asset path the user can change settings like compression and
splitting thresholds.

The config location must be defined in the ASSETS_DIR environment variable during the build process.
A make file that defines that is recommended.

There is a feature called "groups" that allow splitting data into subcategories with their
own settings.

## Future

This system will have many more features added like

- Texture packing to texture array appropriate textures or texture atlasses

- Advanced group naming

- More caching options

- Missing asset placeholders

- Asset specific optimizations

- Raw asset data retrieval
