/**
 * Security check: Only allow safe URLs to prevent protocol-based attacks (e.g. javascript:)
 */
export function isSafeExternalUrl(url: string | undefined): boolean {
  if (!url) {
    return false;
  }
  const lowerUrl = url.toLowerCase();

  // Scheme-relative URLs like `//evil.com` shouldn't be treated as basic root paths
  const isRootRelativeUrl =
    lowerUrl.startsWith('/') && !lowerUrl.startsWith('//');

  return (
    lowerUrl.startsWith('http://') ||
    lowerUrl.startsWith('https://') ||
    lowerUrl.startsWith('mailto:') ||
    lowerUrl.startsWith('tel:') ||
    lowerUrl.startsWith('#') ||
    isRootRelativeUrl
  );
}
