import http.server
import socketserver

class CORSHTTPRequestHandler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        # Add CORS headers
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Cross-Origin-Embedder-Policy', 'require-corp')
        self.send_header('Cross-Origin-Opener-Policy', 'same-origin')
        #self.send_header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
        #self.send_header('Access-Control-Allow-Headers', 'Content-Type, X-Requested-With')
        #self.send_header('X-Frame-Options', 'DENY')
        #self.send_header('X-Content-Type-Options', 'nosniff')
        #self.send_header('Referrer-Policy', 'no-referrer')
        #self.send_header('Permissions-Policy', 'document-domain=()')
        self.send_header('Content-Security-Policy', "worker-src 'self'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval' challenges.cloudflare.com static.cloudflareinsights.com; frame-ancestors 'none';")
        super().end_headers()
    
    def do_OPTIONS(self):
        # Handle OPTIONS requests for CORS preflight
        self.send_response(200)
        self.end_headers()

    def log_message(self, format, *args):
        print(f"{self.address_string()} - {self.log_date_time_string()} - {format % args}")

# Set up the server
port = 8080
handler = CORSHTTPRequestHandler

with socketserver.TCPServer(("", port), handler) as httpd:
    print(f"Serving at port {port}")
    httpd.serve_forever()