// Reference plugin for the PS-A/PS-B host (dev mode).
// Commands show up in the command palette, ribbons in the sidebar dock.
export default class DemoPlugin {
  async onload(ctx) {
    const nodes = await ctx.nodes.listChildren();
    const docs = nodes.filter(node => node.role === 3).length;
    ctx.ui.notice(`Demo plugin: ${docs} document(s) in this workspace`, { type: 'success' });

    ctx.ui.command('count', 'Demo: count documents', async () => {
      const latest = await ctx.nodes.listChildren();
      ctx.ui.notice(`${latest.filter(node => node.role === 3).length} document(s)`);
    });

    ctx.ui.ribbon('🧩', 'Demo plugin', () => {
      ctx.ui.notice(`Hello from ${ctx.manifest.name} v${ctx.manifest.version}`);
    });

    console.info('[demo-plugin] host', ctx.api.host, 'manifest', ctx.manifest.id);
  }

  onunload() {
    console.info('[demo-plugin] unloaded');
  }
}
