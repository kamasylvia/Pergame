// One note per day: `YYYY-MM-DD` under a parent folder stored in settings.
function todayKey() {
  const now = new Date();
  const pad = value => String(value).padStart(2, '0');
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

export default class DailyNote {
  async onload(ctx) {
    ctx.ui.command('open', 'Daily note: open or create today', async () => {
      const key = todayKey();
      const nodes = await ctx.nodes.listChildren();
      let parentId = await ctx.settings.get('parent_id');
      if (parentId && !nodes.some(node => node.id === parentId)) parentId = undefined;

      const existing = nodes.find(node => node.role === 3 && node.name === key && (parentId ? node.parent_id === parentId : true));
      if (existing) {
        ctx.ui.notice(`Today's note exists: ${key}`, { type: 'info' });
        return;
      }
      const created = await ctx.nodes.create({
        name: key,
        parent_id: parentId,
        content: `# ${key}\n\n`,
        role: 3,
      });
      await ctx.settings.set('last_created', created.id);
      ctx.ui.notice(`Created ${key}`, { type: 'success' });
    });

    ctx.ui.command('pick-folder', 'Daily note: use the current folder', async () => {
      // No folder-picker API in PS-A/B: take the first workspace as the home.
      const nodes = await ctx.nodes.listChildren();
      const workspace = nodes.find(node => node.role === 1);
      if (!workspace) {
        ctx.ui.notice('Create a workspace first', { type: 'warning' });
        return;
      }
      await ctx.settings.set('parent_id', workspace.id);
      ctx.ui.notice(`Daily notes go to ${workspace.name}`, { type: 'success' });
    });
  }
}
