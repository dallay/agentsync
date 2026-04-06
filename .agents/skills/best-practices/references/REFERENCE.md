# Code quality

## Valid HTML

```html
<!-- ❌ Invalid HTML -->
<div id="header">
<div id="header"> <!-- Duplicate ID -->

<ul>
  <div>Item</div> <!-- Invalid child -->
</ul>

<a href="/"><button>Click</button></a> <!-- Invalid nesting -->

<!-- ✅ Valid HTML -->
<header id="site-header">
</header>

<ul>
  <li>Item</li>
</ul>

<a href="/" class="button">Click</a>
```

## Semantic HTML

```html
<!-- ❌ Non-semantic -->
<div class="header">
  <div class="nav">
    <div class="nav-item">Home</div>
  </div>
</div>
<div class="main">
  <div class="article">
    <div class="title">Headline</div>
  </div>
</div>

<!-- ✅ Semantic HTML5 -->
<header>
  <nav>
    <a href="/">Home</a>
  </nav>
</header>
<main>
  <article>
    <h1>Headline</h1>
  </article>
</main>
```

## Image aspect ratios

```html
<!-- ❌ Distorted images -->
<img src="photo.jpg" width="300" height="100">
<!-- If actual ratio is 4:3, this squishes the image -->

<!-- ✅ Preserve aspect ratio -->
<img src="photo.jpg" width="300" height="225">
<!-- Actual 4:3 dimensions -->

<!-- ✅ CSS object-fit for flexibility -->
<img src="photo.jpg" style="width: 300px; height: 200px; object-fit: cover;">
```

---

## Permissions & privacy

### Request permissions properly

```javascript
// ❌ Request on page load (bad UX, often denied)
navigator.geolocation.getCurrentPosition(success, error);

// ✅ Request in context, after user action
findNearbyButton.addEventListener('click', async () => {
  // Explain why you need it
  if (await showPermissionExplanation()) {
    navigator.geolocation.getCurrentPosition(success, error);
  }
});
```

### Permissions policy

Set `Permissions-Policy` as an **HTTP response header**, not a meta tag. Browsers enforce this
policy from server responses.

```nginx
# Restrict powerful features
add_header Permissions-Policy "geolocation=(), camera=(), microphone=()" always;

# Or allow a specific origin
add_header Permissions-Policy "geolocation=(self \"https://maps.example.com\")" always;
```

```javascript
// Express / Node example
app.use((_req, res, next) => {
  res.setHeader(
    "Permissions-Policy",
    'geolocation=(), camera=(), microphone=()'
  );
  next();
});
```

---

## Audit checklist

### Security (critical)

- [ ] HTTPS enabled, no mixed content
- [ ] No vulnerable dependencies (`npm audit`)
- [ ] CSP headers configured
- [ ] Security headers present
- [ ] No exposed source maps

### Compatibility

- [ ] Valid HTML5 doctype
- [ ] Charset declared first in head
- [ ] Viewport meta tag present
- [ ] No deprecated APIs used
- [ ] Passive event listeners for scroll/touch

### Code quality

- [ ] No console errors
- [ ] Valid HTML (no duplicate IDs)
- [ ] Semantic HTML elements used
- [ ] Proper error handling
- [ ] Memory cleanup in components

### UX

- [ ] No intrusive interstitials
- [ ] Permission requests in context
- [ ] Clear error messages
- [ ] Appropriate image aspect ratios

## Tools

| Tool                                               | Purpose                    |
|----------------------------------------------------|----------------------------|
| `npm audit`                                        | Dependency vulnerabilities |
| [SecurityHeaders.com](https://securityheaders.com) | Header analysis            |
| [W3C Validator](https://validator.w3.org)          | HTML validation            |
| Lighthouse                                         | Best practices audit       |
| [Observatory](https://observatory.mozilla.org)     | Security scan              |

## References

- [MDN Web Security](https://developer.mozilla.org/en-US/docs/Web/Security)
- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [Web Quality Audit skill](../../web-quality-audit/SKILL.md)
