// 配置
const BACKEND_API_URL = 'http://127.0.0.1:3000/verify';

// 格式化证书数据显示
function formatCertificateData(data) {
    const excludeKeys = ['images']; // 排除不需要直接显示的字段
    let html = '<div class="certificate-data">';
    
    // 显示基本信息
    Object.entries(data).forEach(([key, value]) => {
        if (!excludeKeys.includes(key)) {
            html += `
                <div class="data-item">
                    <span class="data-label">${key}:</span>
                    <span class="data-value">${value}</span>
                </div>
            `;
        }
    });
    
    html += '</div>';
    return html;
}

// 显示图片
function displayImages(images) {
    if (!images || images.length === 0) {
        return '<p>无图片数据</p>';
    }

    let html = '<div class="images-container">';
    images.forEach(image => {
        html += `
            <div class="image-item">
                <h4>${image.name}</h4>
                <img src="data:image/jpeg;base64,${image.data}" alt="${image.name}" />
                <div class="image-info">
                    <p>IPFS CID: ${image.ipfs_cid}</p>
                    <p>大小: ${(image.size / 1024).toFixed(2)} KB</p>
                </div>
            </div>
        `;
    });
    html += '</div>';
    return html;
}

// 验证证书文件
async function verifyCertificate(file) {
    try {
        const reader = new FileReader();
        reader.onload = async function(e) {
            const content = JSON.parse(e.target.result);
            const verifyResult = document.getElementById('verifyResult');
            
            if (!content.tx_hash) {
                throw new Error('证书文件缺少交易哈希(tx_hash)字段');
            }

            if (!content.original_data) {
                throw new Error('证书文件缺少原始数据(original_data)字段');
            }

            try {
                // 发送交易哈希和原始数据到后端进行验证
                const response = await fetch(BACKEND_API_URL, {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({
                        tx_hash: content.tx_hash,
                        original_data: content.original_data
                    })
                });

                if (!response.ok) {
                    throw new Error(`验证失败: ${response.statusText}`);
                }

                const result = await response.json();
                
                if (result.verified) {
                    const certificateData = result.data.original_data;
                    const images = result.data.images;

                    verifyResult.innerHTML = `
                        <div class="success-message">
                            <h3>✓ 证书验证成功</h3>
                            <p>交易哈希: ${content.tx_hash}</p>
                            
                            <div class="certificate-details">
                                <h4>证书信息</h4>
                                ${formatCertificateData(certificateData)}
                                
                                <h4>证书图片</h4>
                                ${displayImages(images)}
                            </div>
                        </div>
                    `;

                    // 添加样式
                    const style = document.createElement('style');
                    style.textContent = `
                        .certificate-details {
                            margin-top: 20px;
                            padding: 20px;
                            border: 1px solid #ddd;
                            border-radius: 5px;
                        }
                        .certificate-data {
                            display: grid;
                            grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
                            gap: 10px;
                            margin-bottom: 20px;
                        }
                        .data-item {
                            padding: 10px;
                            background: #f5f5f5;
                            border-radius: 4px;
                        }
                        .data-label {
                            font-weight: bold;
                            margin-right: 10px;
                        }
                        .images-container {
                            display: grid;
                            grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
                            gap: 20px;
                            margin-top: 20px;
                        }
                        .image-item {
                            border: 1px solid #ddd;
                            padding: 10px;
                            border-radius: 5px;
                        }
                        .image-item img {
                            max-width: 100%;
                            height: auto;
                            border-radius: 4px;
                        }
                        .image-info {
                            margin-top: 10px;
                            font-size: 0.9em;
                            color: #666;
                        }
                        .success-message {
                            color: #155724;
                            background-color: #d4edda;
                            border: 1px solid #c3e6cb;
                            padding: 20px;
                            border-radius: 5px;
                            margin-bottom: 20px;
                        }
                        .error-message {
                            color: #721c24;
                            background-color: #f8d7da;
                            border: 1px solid #f5c6cb;
                            padding: 20px;
                            border-radius: 5px;
                        }
                    `;
                    document.head.appendChild(style);
                } else {
                    verifyResult.innerHTML = `
                        <div class="error-message">
                            <h3>✗ 证书验证失败</h3>
                            <p>原因: ${result.message}</p>
                        </div>
                    `;
                }
            } catch (fetchError) {
                console.error('验证失败:', fetchError);
                verifyResult.innerHTML = `
                    <div class="error-message">
                        <h3>✗ 验证失败</h3>
                        <p>错误信息: ${fetchError.message}</p>
                    </div>
                `;
            }
        };
        reader.readAsText(file);
    } catch (error) {
        console.error('验证失败:', error);
        document.getElementById('verifyResult').innerHTML = `
            <div class="error-message">
                <h3>✗ 验证失败</h3>
                <p>错误信息: ${error.message}</p>
            </div>
        `;
    }
}

// 监听验证文件输入
document.getElementById('verifyInput').addEventListener('change', async (e) => {
    const file = e.target.files[0];
    if (file) {
        await verifyCertificate(file);
    }
}); 