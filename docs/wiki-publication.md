# Wiki publication

The [public wiki](https://github.com/qunevo/apex/wiki) is generated from reviewed repository documentation. Edit the source files through a pull request. No AI service, model API, personal access token or third-party publishing action is used by the workflow.

## Source and publication

`scripts/wiki-pages.json` is the explicit public-page allowlist and navigation order. Page labels come from each document's first level-one heading. `docs/README.md` becomes `Home`; `_Sidebar.md` and `_Footer.md` are generated. New files are not automatically published. Review a new page's contents before adding it to the allowlist.

`python scripts/build_wiki.py` validates the allowlist and local Markdown link targets without writing files. The exporter supports inline links and images, reference link definitions, fragments and link titles. Fenced and inline code examples are preserved. It does not fetch external URLs or rewrite HTML links; use Markdown links in published documents. Links between exported documents lead to wiki pages. Links to source, examples, schemas and the canonical license point to the exact source commit on GitHub.

For a local preview:

```bash
python -m unittest discover -s tests -p 'test_wiki.py' -v
python scripts/build_wiki.py --output build/wiki
```

Generated pages are not committed to the main repository. The wiki keeps ordinary Git history. The exporter records managed filenames, removes obsolete managed pages and preserves unrelated pages. It refuses symlinks and refuses to overwrite pages without its generated marker. On first adoption, review existing pages before marking them as generated; do not silently discard handwritten content.

## GitHub setup

Enable Wikis and keep editing restricted to collaborators with push access. Initialize the wiki by creating its first `Home` page in GitHub; GitHub requires this before its separate Git repository can be cloned. The initial generated Home page must start with the exporter's marker.

The `Wiki` workflow validates pull requests with read-only permissions. Publication runs only on `main` in `qunevo/apex`, after a push or a manual run on `main`. It uses the job-scoped GitHub Actions token with `contents: write` to check out and push `qunevo/apex.wiki`. No reusable credential is stored in this repository. A failed wiki checkout or push fails the job visibly; it never silently reports success or falls back to a broader credential. Check that the wiki is initialized and that organization policy permits the job's write permission if authentication fails.

Publication runs are serialized and read the latest `main` revision when they start. That revision is tested again before publication, so an older queued run cannot roll documentation back. There are no force pushes. Re-running an unchanged revision creates no wiki commit. A failed run can be retried from the Actions page after correcting its cause.

The general CI's `Source publication` job also runs the wiki tests and link validation. Its result is included in the required `Required checks` aggregate, so invalid documentation blocks merging. Wiki edits do not use the main repository's pull-request review flow; maintainers should submit documentation changes to the main repository instead. Direct edits to generated wiki pages are replaced during publication. Review and merge requirements for `main` remain in force.

The website's Documentation link points to the wiki. The maintained documentation is English; the website's navigation labels are available in German and English.
