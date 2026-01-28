# Deployment 🚀

## What ships

- Source tree only
- No `.git/`
- No raw images
- No cached downloads

This keeps releases small, auditable, and reproducible.

---

## Packaging

```bash
tar -czf MASH_GIT.tar.gz MASH_GIT/
```

---

## Testing guidance

- Always test on removable media first
- Prefer **MBR** unless testing firmware edge cases
- Expect destructive behaviour — this is intentional
