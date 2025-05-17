from flask import Flask, render_template, send_from_directory
import os

app = Flask(__name__)

# 打印当前工作目录和模板目录
print("Current working directory:", os.getcwd())
print("Templates directory:", os.path.join(os.getcwd(), 'templates'))

# 定义路由
@app.route('/')
def index():
    print("Accessing index page")
    return render_template('index.html')

@app.route('/student/generate')
def student_generate():
    print("Accessing student generate page")
    return render_template('student_generate_authentication.html')

@app.route('/student/upload')
def student_upload():
    print("Accessing student upload page")
    return render_template('student_upload_data.html')

@app.route('/school/check')
def school_check():
    print("Accessing school check page")
    return render_template('school_data_check.html')

@app.route('/school/upload')
def school_upload():
    print("Accessing school upload page")
    return render_template('school_upload_data.html')

@app.route('/company/upload')
def company_verify():
    print("Accessing company verify page")
    return render_template('company_submit_verification.html')

@app.route('/company/result')
def company_result():
    print("Accessing company result page")
    return render_template('company_confirm_result.html')

# 静态文件路由
@app.route('/static/<path:filename>')
def serve_static(filename):
    return send_from_directory('static', filename)

if __name__ == '__main__':
    app.run(debug=True, port=8000, host='0.0.0.0') 