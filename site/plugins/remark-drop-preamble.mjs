// The site renders CHANGELOG.md inside its own page, which carries its own title.
// Drop the file's preamble (the `# ` title) so content starts at the first `## `.
export default function remarkDropPreamble() {
  return (tree) => {
    const start = tree.children.findIndex((n) => n.type === 'heading' && n.depth === 2);
    if (start > 0) tree.children.splice(0, start);
  };
}
