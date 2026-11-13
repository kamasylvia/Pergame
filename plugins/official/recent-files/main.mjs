// Lists the five most recently updated documents in the caller's library.
const LIMIT = 5;

async function recent(ctx) {
  const nodes = await ctx.nodes.listChildren();
  return nodes
    .filter(node => node.role === 3)
    .sort((a, b) => b.updated_timestamp - a.updated_timestamp)
    .slice(0, LIMIT);
}

export default class RecentFiles {
  async onload(ctx) {
    ctx.ui.command('show', 'Recent files: show last edited', async () => {
      const docs = await recent(ctx);
      if (!docs.length) {
        ctx.ui.notice('No documents yet');
        return;
      }
      ctx.ui.notice(docs.map(doc => doc.name).join(' · '), { timeout: 8000 });
    });

    ctx.ui.ribbon('🕘', 'Recent files', async () => {
      const docs = await recent(ctx);
      if (!docs.length) {
        ctx.ui.notice('No documents yet');
        return;
      }
      const oldest = docs[docs.length - 1].updated_timestamp;
      const minutes = Math.max(0, Math.round((Date.now() - oldest) / 60000));
      ctx.ui.notice(`${docs.length} recent document(s), oldest ${minutes} min ago`);
    });
  }
}
