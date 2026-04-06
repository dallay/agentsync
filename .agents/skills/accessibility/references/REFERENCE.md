# Testing checklist

## Automated testing

```bash
# Lighthouse accessibility audit
npx lighthouse https://example.com --only-categories=accessibility

# axe-core
npm install @axe-core/cli -g
axe https://example.com
```

## Manual testing

- [ ] **Keyboard navigation:** Tab through entire page, use Enter/Space to activate
- [ ] **Screen reader:** Test with VoiceOver (Mac), NVDA (Windows), or TalkBack (Android)
- [ ] **Zoom:** Content usable at 200% zoom
- [ ] **High contrast:** Test with Windows High Contrast Mode
- [ ] **Reduced motion:** Test with `prefers-reduced-motion: reduce`
- [ ] **Focus order:** Logical and follows visual order

## Screen reader commands

| Action        | VoiceOver (Mac)     | NVDA (Windows) |
|---------------|---------------------|----------------|
| Start/Stop    | ⌘ + F5              | Ctrl + Alt + N |
| Next item     | VO + →              | ↓              |
| Previous item | VO + ←              | ↑              |
| Activate      | VO + Space          | Enter          |
| Headings list | VO + U, then arrows | H / Shift + H  |
| Links list    | VO + U              | K / Shift + K  |

---

## Common issues by impact

### Critical (fix immediately)

1. Missing form labels
2. Missing image alt text
3. Insufficient color contrast
4. Keyboard traps
5. No focus indicators

### Serious (fix before launch)

1. Missing page language
2. Missing heading structure
3. Non-descriptive link text
4. Auto-playing media
5. Missing skip links

### Moderate (fix soon)

1. Missing ARIA labels on icons
2. Inconsistent navigation
3. Missing error identification
4. Timing without controls
5. Missing landmark regions

## References

- [WCAG 2.1 Quick Reference](https://www.w3.org/WAI/WCAG21/quickref/)
- [WAI-ARIA Authoring Practices](https://www.w3.org/WAI/ARIA/apg/)
- [Deque axe Rules](https://dequeuniversity.com/rules/axe/)
- [Web Quality Audit](../web-quality-audit/SKILL.md)
