export function getAuthorizeHtml(authParams: Record<string, string>): string {
    return `
                <!DOCTYPE html>
                <html>
                <head>
                    <title>Fake Login</title>
                </head>
                <script>
                    function createUrlQueryString(params) {
                        console.log(params);
                        const p = [];
                        for (const k of Object.keys(params)) {
                            p.push(\`\${k}=\${encodeURIComponent(params[k])}\`);
                        }
                        return p.join('&');
                    }

                    function generateUrl(id) {
                        console.log(\`userId: \${id}\`);

                        const state = '${authParams.state}';
                        const name = \`oauth_\${id}\`;
                        const email = \`\${name}@example.com\`;

                        const code = encodeURIComponent(
                            createUrlQueryString({
                                id,
                                name,
                                email,
                                ${authParams.nonce ? `nonce: '${authParams.nonce}'` : ''}
                            })
                        );

                        const redirectUrl=\`${authParams.redirect_uri}?state=\${state}&code=\${code}\`;
                        console.log(\`redirectUrl: \${redirectUrl}\`);

                        return redirectUrl;
                    }

                    document.addEventListener('DOMContentLoaded', function() {
                        const userIdInput = document.getElementById('userId');
                        const loginLink = document.getElementById('link');

                        userIdInput.addEventListener('input', function() {
                            const updatedId = userIdInput.value;
                            loginLink.href = generateUrl(updatedId);
                        });
                    });
                </script>
                <body>
                    <input type="text" id="userId" placeholder="Enter ID" />
                    <a id="link" href="">Login</a>
                </body>
                </html>
            `;
}
