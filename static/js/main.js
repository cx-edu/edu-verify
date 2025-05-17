// 存储处理后的数据
const processedData = new Map();

// 发送数据到后端的函数
async function sendToBackend(data) {
    try {
        // 转换数据格式以匹配后端期望的结构
        const formattedData = data.map(item => ({
            id: item.id,
            data: item.data
        }));

        console.log('Sending data to backend:', formattedData);
        
        const response = await fetch('http://localhost:3000/api/school/upload', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify(formattedData)
        });

        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }

        const result = await response.json();
        console.log('Backend response:', result);
        
        // 显示成功消息和下载按钮，但保留文件预览
        if (result.success) {
            const previewDiv = document.getElementById('preview');
            const resultDiv = document.createElement('div');
            resultDiv.className = 'success-message';
            resultDiv.style.marginBottom = '20px';
            resultDiv.innerHTML = `
                <h3>✓ 证书生成成功</h3>
                <button onclick="downloadCertificate('${result.certificate_file}')" class="download-btn">
                    下载证书
                </button>
            `;
            // 在预览区域的开头插入成功消息
            previewDiv.insertBefore(resultDiv, previewDiv.firstChild);
        } else {
            throw new Error('证书生成失败');
        }
    } catch (error) {
        console.error('Error sending data to backend:', error);
        const previewDiv = document.getElementById('preview');
        const errorDiv = document.createElement('div');
        errorDiv.className = 'error-message';
        errorDiv.style.marginBottom = '20px';
        errorDiv.innerHTML = `
            <h3>✗ 上传失败</h3>
            <p>${error.message}</p>
        `;
        previewDiv.insertBefore(errorDiv, previewDiv.firstChild);
    }
}

// 添加下载证书的函数
async function downloadCertificate(filename) {
    try {
        const response = await fetch(`http://localhost:3000/api/school/download/${filename}`);
        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }
        
        // 获取blob数据
        const blob = await response.blob();
        
        // 创建下载链接
        const url = window.URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = filename;
        
        // 触发下载
        document.body.appendChild(a);
        a.click();
        
        // 清理
        window.URL.revokeObjectURL(url);
        document.body.removeChild(a);
    } catch (error) {
        console.error('Error downloading certificate:', error);
        alert('下载失败: ' + error.message);
    }
}

// 处理Excel文件的函数
async function processExcelFile(file) {
    try {
        const arrayBuffer = await file.arrayBuffer();
        const data = new Uint8Array(arrayBuffer);
        const workbook = XLSX.read(data, { type: 'array' });
        
        // 获取第一个工作表
        const firstSheet = workbook.Sheets[workbook.SheetNames[0]];
        const jsonData = XLSX.utils.sheet_to_json(firstSheet, { header: 1 });
        
        if (jsonData.length < 2) return; // 确保至少有表头和一行数据
        
        const headers = jsonData[0];
        
        // 处理每一行数据
        for (let i = 1; i < jsonData.length; i++) {
            const row = jsonData[i];
            const rowData = {};
            
            // 假设第一列是ID
            const id = row[0];
            if (!id) continue;
            
            // 将每一列的数据与表头对应，确保所有值都是字符串
            headers.forEach((header, index) => {
                const value = row[index];
                // 将所有值转换为字符串，如果值不存在则设为空字符串
                rowData[header.toString()] = value != null ? value.toString() : '';
            });
            
            // 存储到Map中
            processedData.set(id.toString(), {
                data: {
                    ...rowData,
                    images: [] // 图片信息放入 data 中
                }
            });
        }

        console.log('Processed Excel data:', processedData);
    } catch (error) {
        console.error('处理Excel文件时出错:', error);
    }
}

// 处理图片文件的函数
function processImageFile(file) {
    const fileName = file.name;
    // 假设图片文件名中包含ID（例如：123.jpg 中的123就是ID）
    const id = fileName.split('.')[0];
    
    if (processedData.has(id)) {
        console.log(`Processing image for ID ${id}`);
        const reader = new FileReader();
        reader.onload = function(e) {
            console.log(`FileReader loaded for image ${fileName}`);
            const imageData = {
                name: fileName,
                url: e.target.result,
                data: e.target.result.split(',')[1]
            };
            console.log(`Image data length: ${imageData.data.length}`);
            
            const existingData = processedData.get(id.toString());
            console.log('Existing data before adding image:', JSON.stringify(existingData));
            
            existingData.data.images.push({
                name: fileName,
                url: imageData.url,
                data: imageData.data
            });
            
            console.log('Updated data after adding image:', JSON.stringify(existingData));
            updatePreview(id, imageData);
        };
        reader.readAsDataURL(file);
    } else {
        console.log(`No matching ID ${id} found in processedData for image ${fileName}`);
    }
}

// 更新预览区域
function updatePreview(id, imageData) {
    const previewDiv = document.getElementById('preview');
    const data = processedData.get(id);
    
    // 创建或更新预览元素
    let itemDiv = document.getElementById(`preview-${id}`);
    if (!itemDiv) {
        itemDiv = document.createElement('div');
        itemDiv.id = `preview-${id}`;
        itemDiv.style.marginBottom = '20px';
        previewDiv.appendChild(itemDiv);
    }
    
    // 显示数据和图片
    itemDiv.innerHTML = `
        <h3>ID: ${id}</h3>
        <pre>${JSON.stringify(data.data, null, 2)}</pre>
        <div class="images">
            ${data.data.images.map(img => `<img src="${img.url}" alt="${img.name}">`).join('')}
        </div>
    `;
}

// 处理文件夹上传
async function handleFiles(files) {
    const excelFiles = Array.from(files).filter(file => 
        file.name.endsWith('.xlsx') || file.name.endsWith('.xls'));
    const imageFiles = Array.from(files).filter(file => 
        file.type.startsWith('image/'));
    
    // 先处理Excel文件
    for (const file of excelFiles) {
        await processExcelFile(file);
    }
    
    // 修改图片处理为Promise形式
    const processImage = (file) => {
        return new Promise((resolve) => {
            const fileName = file.name;
            const id = fileName.split('.')[0];
            
            if (processedData.has(id)) {
                console.log(`Processing image for ID ${id}`);
                const reader = new FileReader();
                reader.onload = function(e) {
                    console.log(`FileReader loaded for image ${fileName}`);
                    const imageData = {
                        name: fileName,
                        url: e.target.result,
                        data: e.target.result.split(',')[1]
                    };
                    console.log(`Image data length: ${imageData.data.length}`);
                    
                    const existingData = processedData.get(id.toString());
                    console.log('Existing data before adding image:', JSON.stringify(existingData));
                    
                    existingData.data.images.push({
                        name: fileName,
                        url: imageData.url,
                        data: imageData.data
                    });
                    
                    console.log('Updated data after adding image:', JSON.stringify(existingData));
                    updatePreview(id, imageData);
                    resolve();
                };
                reader.readAsDataURL(file);
            } else {
                console.log(`No matching ID ${id} found in processedData for image ${fileName}`);
                resolve();
            }
        });
    };

    // 等待所有图片处理完成
    await Promise.all(imageFiles.map(file => processImage(file)));
    
    // 如果有数据，显示上传按钮
    if (processedData.size > 0) {
        const previewDiv = document.getElementById('preview');
        const uploadButton = document.createElement('button');
        uploadButton.textContent = '上传数据';
        uploadButton.onclick = () => {
            const dataArray = Array.from(processedData.entries()).map(([id, data]) => ({
                id,
                ...data
            }));
            sendToBackend(dataArray);
        };
        previewDiv.appendChild(uploadButton);
    }
}

// 处理文件夹树
async function traverseFileTree(item, path = '') {
    if (item.isFile) {
        const file = await new Promise(resolve => item.file(resolve));
        if (file.name.endsWith('.xlsx') || file.name.endsWith('.xls')) {
            await processExcelFile(file);
        } else if (file.type.startsWith('image/')) {
            processImageFile(file);
        }
    } else if (item.isDirectory) {
        const dirReader = item.createReader();
        await new Promise(resolve => {
            dirReader.readEntries(async entries => {
                for (const entry of entries) {
                    await traverseFileTree(entry, path + item.name + "/");
                }
                resolve();
            });
        });
    }
}

// 设置拖放事件监听器
const uploadArea = document.getElementById('uploadArea');

uploadArea.addEventListener('dragover', (e) => {
    e.preventDefault();
    e.stopPropagation();
    uploadArea.style.borderColor = '#666';
});

uploadArea.addEventListener('dragleave', (e) => {
    e.preventDefault();
    e.stopPropagation();
    uploadArea.style.borderColor = '#ccc';
});

uploadArea.addEventListener('drop', async (e) => {
    e.preventDefault();
    e.stopPropagation();
    uploadArea.style.borderColor = '#ccc';
    
    const items = e.dataTransfer.items;
    if (items) {
        for (const item of items) {
            if (item.webkitGetAsEntry) {
                const entry = item.webkitGetAsEntry();
                if (entry) {
                    await traverseFileTree(entry);
                }
            }
        }
    } else {
        handleFiles(e.dataTransfer.files);
    }
});

// 监听文件选择
document.getElementById('folderInput').addEventListener('change', (e) => {
    handleFiles(e.target.files);
}); 