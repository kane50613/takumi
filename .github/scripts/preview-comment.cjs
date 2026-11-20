// Posts the docs preview URL on the pull request, or updates the comment that already holds it.
module.exports = async ({ github, context }) => {
  const marker = "<!-- docs-preview-url -->";
  const body = `${marker}\nDocs preview is ready: ${process.env.PREVIEW_URL}`;
  const issue = {
    owner: context.repo.owner,
    repo: context.repo.repo,
    issue_number: context.issue.number,
  };
  const comments = await github.paginate(github.rest.issues.listComments, issue);
  const existing = comments.find((comment) => comment.body?.includes(marker));

  if (existing) {
    await github.rest.issues.updateComment({ ...issue, comment_id: existing.id, body });
  } else {
    await github.rest.issues.createComment({ ...issue, body });
  }
};
