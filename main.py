from flask import *

app = Flask(__name__)

@app.route("/")
@app.route("/index.html")
def index():
    return render_template("index.html")

@app.route("/license")
@app.route("/license.html")
def license():
    return render_template("license.html")

@app.errorhandler(404)
def not_found(e):
    return render_template("404.html")

@app.route('/favicon.ico')
def favicon():
    return send_from_directory("favicon.ico",
        'favicon.ico',mimetype='image/vnd.microsoft.icon')

if __name__ == "__main__":
    app.run(debug=False)