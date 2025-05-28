import http.server
import socketserver

class CORSHTTPRequestHandler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        # Add CORS headers
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', 'Content-Type, X-Requested-With')
        # Call the original end_headers method
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
    print(f"Serving with CORS enabled at port {port}")
    httpd.serve_forever()