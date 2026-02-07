const EXTERNAL_PROTOCOL_RE = /^[a-zA-Z][a-zA-Z\d+.-]*:/;

const isExternalLink = (href: string): boolean =>
	EXTERNAL_PROTOCOL_RE.test(href) || href.startsWith("//");

const normalizeBase = (base: string): string =>
	base.endsWith("/") ? base.slice(0, -1) : base;

const normalizeHref = (href: string): string =>
	href.startsWith("/") ? href : `/${href}`;

export const withBase = (href?: string): string | undefined => {
	if (!href) {
		return href;
	}

	if (href.startsWith("#") || href.startsWith("?") || isExternalLink(href)) {
		return href;
	}

	const base = import.meta.env.BASE_URL ?? "/";
	const normalizedBase = normalizeBase(base);
	const normalizedHref = normalizeHref(href);

	if (
		normalizedBase &&
		(normalizedHref === normalizedBase ||
			normalizedHref.startsWith(`${normalizedBase}/`))
	) {
		return normalizedHref;
	}

	if (!normalizedBase) {
		return normalizedHref;
	}

	return `${normalizedBase}${normalizedHref}`;
};
