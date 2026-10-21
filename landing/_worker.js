// Canonical apex only: www → vanity-address.online; real 404 for missing paths.
const CANONICAL = "vanity-address.online";

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.hostname === `www.${CANONICAL}`) {
      url.protocol = "https:";
      url.hostname = CANONICAL;
      url.port = "";
      return Response.redirect(url.toString(), 301);
    }

    const res = await env.ASSETS.fetch(request);
    if (res.status !== 404) return res;

    const notFound = await env.ASSETS.fetch(new URL("/404.html", url.origin));
    return new Response(notFound.body, {
      status: 404,
      statusText: "Not Found",
      headers: notFound.headers,
    });
  },
};
