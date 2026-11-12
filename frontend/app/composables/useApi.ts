import type { Node, PublicUser, User } from '~/stores';

export function useApi() {
  const config = useRuntimeConfig();
  const API = `${config.public.baseApi}/api`;

  function avatarURL(user?: User | PublicUser | null): string {
    if (!user?.avatar) return '/placeholder/avatar.avif';
    if (user.avatar.startsWith('http://') || user.avatar.startsWith('https://')) {
      return user.avatar;
    }
    return `${API}/users/${user.id}/avatar?v=${user.avatar}`;
  }

  function resourceURL(resource?: Node, download = false): string {
    if (!resource) return '';
    return `${API}/resources/${resource.id}/file${download ? '?download=true' : ''}`;
  }

  return { API, avatarURL, resourceURL };
}
