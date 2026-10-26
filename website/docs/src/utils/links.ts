const EXTERNAL_PROTOCOL_RE = /^[a-zA-Z][a-zA-Z\d+.-]*:/;

const isExternalLink = (href: string): boolean =>
	EXTERNAL_PROTOCOL_RE.test(href) || href.startsWith("//");

const normalizeBase = (base: string): string =>
	base.endsWith("/") ? base.slice(0, -1) : base;

const normalizeHref = (href: string): string =>
	href.startsWith("/") ? href : `/${href}`;

/**
 * Prefix local paths with the locale before applying the site's base path.
 * Leave fragment and query references, URLs with a scheme, protocol-relative
 * URLs, and paths already prefixed with the requested locale unchanged.
 *
 * @param locale - Locale path segment; undefined or empty leaves href unchanged.
 * @returns The prefixed path, or the original href when no prefix is needed.
 * An empty href with a nonempty locale becomes `/{locale}/`.
 */
export const withLocale = (href: string, locale: string | undefined): string => {
	if (!locale || href.startsWith("#") || href.startsWith("?") || isExternalLink(href)) {
		return href;
	}

	const normalizedHref = normalizeHref(href);
	const localePrefix = `/${locale}`;
	if (normalizedHref === localePrefix || normalizedHref.startsWith(`${localePrefix}/`)) {
		return href;
	}

	return `${localePrefix}${normalizedHref}`;
};

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
