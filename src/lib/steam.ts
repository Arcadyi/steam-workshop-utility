
export function steamHeaderUrl(appid: string): string {
    return `https://cdn.cloudflare.steamstatic.com/steam/apps/${appid}/header.jpg`;
}

export function steamCapsuleUrl(appid: string): string {
    return `https://cdn.cloudflare.steamstatic.com/steam/apps/${appid}/library_600x900.jpg`;
}

export function steamIconUrl(appid: string): string {
    return `https://cdn.cloudflare.steamstatic.com/steam/apps/${appid}/capsule_sm_120.jpg`;
}