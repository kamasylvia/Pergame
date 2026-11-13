// Reference plugin for the PS-A host (dev mode).
// Loads through `pergame`-free ESM: the context is passed to onload.
export default class DemoPlugin {
  async onload(ctx) {
    const nodes = await ctx.nodes.listChildren();
    const docs = nodes.filter(node => node.role === 3).length;
    ctx.ui.notice(`Demo plugin: ${docs} document(s) in this workspace`, { type: 'success' });
    console.info('[demo-plugin] host', ctx.api.host, 'manifest', ctx.manifest.id);
  }

  onunload() {
    console.info('[demo-plugin] unloaded');
  }
}
