# Asset system

Asset system for let-engine that allows you to pack your resources from a asset path
to be output next to your binary.

At runtime those resources can easily be accessed using a function that loads and caches
those resources.

Using a config located at the asset path the user can change settings like compression and
splitting thresholds.

There is a feature called "groups" that allow splitting data into subcategories with their
own settings.

## Future

This system will have many more features added like

- Texture packing to texture array appropriate textures or texture atlasses

- Advanced group naming

- More caching options

- Missing asset placeholders

- Asset specific optimisations

- Raw asset data retrieval
