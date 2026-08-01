const DEFAULT_REPO = 'oMaN-Rod/palworld-save-pal';
const CACHE_KEY = 'psp:gh-stars';

export async function fetchGithubStars(repo: string = DEFAULT_REPO): Promise<number | null> {
	if (typeof sessionStorage !== 'undefined') {
		const cached = sessionStorage.getItem(CACHE_KEY);
		if (cached !== null) {
			const n = Number(cached);
			return Number.isFinite(n) ? n : null;
		}
	}
	try {
		const res = await fetch(`https://api.github.com/repos/${repo}`, {
			headers: { Accept: 'application/vnd.github+json' }
		});
		if (!res.ok) return null;
		const data = await res.json();
		const stars = typeof data?.stargazers_count === 'number' ? data.stargazers_count : null;
		if (stars !== null && typeof sessionStorage !== 'undefined') {
			sessionStorage.setItem(CACHE_KEY, String(stars));
		}
		return stars;
	} catch {
		return null;
	}
}

export function formatStars(n: number): string {
	if (n < 1000) return String(n);
	const k = (n / 1000).toFixed(n < 10000 ? 1 : 0);
	return `${k.replace(/\.0$/, '')}k`;
}
