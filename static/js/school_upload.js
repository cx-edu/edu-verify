// 存储处理后的数据
const processedData = new Map();

// 处理Excel文件的函数
async function processExcelFile(file) {
    try {
        const arrayBuffer = await file.arrayBuffer();
        const data = new Uint8Array(arrayBuffer);
        const workbook = XLSX.read(data, { type: 'array' });
        
        // 获取第一个工作表
        const firstSheet = workbook.Sheets[workbook.SheetNames[0]];
        const jsonData = XLSX.utils.sheet_to_json(firstSheet, { header: 1 });
        
        if (jsonData.length < 2) {
            throw new Error('Excel文件必须至少包含表头和一行数据');
        }
        
        const headers = jsonData[0];
        const certificateNumberIndex = headers.findIndex(header => header.toString() === '证书编号');
        
        if (certificateNumberIndex === -1) {
            throw new Error('Excel文件必须包含"证书编号"列');
        }
        
        // 处理每一行数据
        for (let i = 1; i < jsonData.length; i++) {
            const row = jsonData[i];
            const rowData = {};
            
            // 获取证书编号
            const certificateNumber = row[certificateNumberIndex];
            if (!certificateNumber) continue;
            
            // 将每一列的数据与表头对应
            headers.forEach((header, index) => {
                const value = row[index];
                rowData[header.toString()] = value != null ? value.toString() : '';
            });
            
            // 存储到Map中，使用证书编号作为key
            processedData.set(certificateNumber.toString(), {
                data: rowData,
                images: [] // 图片信息直接放在第一层
            });
        }

        if (processedData.size === 0) {
            throw new Error('Excel文件中没有有效数据');
        }

        console.log('Excel处理完成');
        console.log('处理后的数据:', processedData);
        console.log('可用的证书编号:', Array.from(processedData.keys()));
    } catch (error) {
        console.error('处理Excel文件时出错:', error);
        showError('处理Excel文件时出错: ' + error.message);
        throw error;
    }
}

// 根据证书编号查找对应的数据记录
function findEntryByCertificateNumber(certificateNumber) {
    for (const [id, entry] of processedData.entries()) {
        if (entry.data['证书编号'] === certificateNumber) {
            return { id, entry };
        }
    }
    return null;
}

// 处理图片文件的函数
function processImageFile(file) {
    return new Promise((resolve, reject) => {
        const fileName = file.name;
        // 获取证书编号（文件名去掉扩展名）
        const certificateNumber = fileName.split('.')[0];
        
        console.log('\n开始处理图片:', fileName);
        console.log('提取的证书编号:', certificateNumber);
        console.log('当前所有证书编号:', Array.from(processedData.keys()));
        
        // 直接检查Map中是否存在该证书编号
        if (processedData.has(certificateNumber)) {
            const reader = new FileReader();
            reader.onload = function(e) {
                const imageData = {
                    name: fileName,
                    url: e.target.result,
                    data: e.target.result.split(',')[1]
                };
                
                // 将图片添加到对应记录中
                const record = processedData.get(certificateNumber);
                record.images.push({
                    name: fileName,
                    url: imageData.url,
                    data: imageData.data
                });
                
                console.log(`✓ 成功添加图片 ${fileName} 到证书编号 ${certificateNumber}`);
                resolve();
            };
            reader.onerror = function(e) {
                const errorMsg = `处理图片 ${fileName} 时出错`;
                console.error('✗', errorMsg);
                reject(new Error(errorMsg));
            };
            reader.readAsDataURL(file);
        } else {
            const errorMsg = `未找到证书编号 ${certificateNumber} 对应的学生信息，请确保图片文件名与Excel中的证书编号完全匹配`;
            console.error('✗', errorMsg);
            console.error('可用的证书编号:', Array.from(processedData.keys()));
            reject(new Error(errorMsg));
        }
    });
}

// 显示错误信息
function showError(message) {
    alert(message);
}

// 显示数据表格
function displayDataTable() {
    const uploadArea = document.getElementById('uploadArea');
    if (!uploadArea) {
        console.error('未找到uploadArea元素');
        showError('页面元素错误：未找到上传区域');
        return;
    }

    // 确保上传区域有足够的空间显示表格
    uploadArea.style.minHeight = '400px';
    uploadArea.style.width = '100%';
    uploadArea.style.overflow = 'auto';

    // 检查是否有数据
    if (processedData.size === 0) {
        console.error('没有可显示的数据');
        showError('没有可显示的数据');
        return;
    }

    try {
        console.log('开始处理数据表格显示');
        console.log('当前数据:', processedData);
        
        // 获取第一条数据来确定表头
        const firstEntry = processedData.values().next().value;
        console.log('第一条数据:', firstEntry);

        if (!firstEntry) {
            throw new Error('无法获取第一条数据');
        }
        if (!firstEntry.data) {
            throw new Error('数据格式错误: 缺少data字段');
        }

        // 创建表格
        let tableHtml = `
            <div class="bg-white shadow-md rounded-lg overflow-hidden" style="margin: 20px 0;">
                <div class="px-6 py-4 border-b border-gray-200 bg-gray-50">
                    <h2 class="text-xl font-semibold text-gray-800">数据预览</h2>
                    <p class="text-sm text-gray-600">共 ${processedData.size} 条记录</p>
                </div>
                <div class="overflow-x-auto" style="max-height: 600px;">
                    <table class="min-w-full divide-y divide-gray-200" style="width: 100%;">
                        <thead class="bg-gray-50" style="position: sticky; top: 0;">
                            <tr>
        `;

        // 获取所有字段名
        const headers = Object.keys(firstEntry.data);
        console.log('表头字段:', headers);

        if (headers.length === 0) {
            throw new Error('数据格式错误: 没有可显示的字段');
        }
        
        // 添加表头
        headers.forEach(header => {
            tableHtml += `<th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider" style="background: #f9fafb;">${header}</th>`;
        });
        tableHtml += `<th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider" style="background: #f9fafb;">证书图片</th>`;
        tableHtml += `</tr></thead><tbody class="bg-white divide-y divide-gray-200">`;

        // 添加数据行
        let rowCount = 0;
        for (const [certificateNumber, entry] of processedData.entries()) {
            console.log(`处理证书编号 ${certificateNumber} 的数据:`, entry);

            if (!entry || !entry.data) {
                console.warn(`跳过无效数据: ${certificateNumber}`, entry);
                continue;
            }

            rowCount++;
            tableHtml += `<tr class="${rowCount % 2 === 0 ? 'bg-gray-50' : 'bg-white'}">`;
            headers.forEach(header => {
                const value = entry.data[header] || '';
                tableHtml += `<td class="px-6 py-4 whitespace-nowrap text-sm text-gray-900">${value}</td>`;
            });

            // 添加图片预览
            const images = entry.images || [];
            tableHtml += `
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
                    ${images.length > 0 ? 
                        `<button onclick="previewImage('${images[0].url}')" class="text-blue-600 hover:text-blue-800 px-3 py-1 rounded border border-blue-600 hover:bg-blue-50">
                            查看图片
                        </button>` : 
                        '<span class="text-red-500 px-2 py-1 bg-red-50 rounded">未上传</span>'}
                </td>
            `;
            tableHtml += `</tr>`;
        }

        tableHtml += `</tbody></table></div>`;
        
        // 添加数据统计
        tableHtml += `
            <div class="px-6 py-4 border-t border-gray-200 bg-gray-50">
                <p class="text-sm text-gray-600">
                    实际显示 ${rowCount} 条记录
                    ${rowCount !== processedData.size ? `（${processedData.size - rowCount} 条记录因数据无效被跳过）` : ''}
                </p>
            </div>
        </div>`;

        // 添加操作按钮
        tableHtml += `
            <div class="mt-6 flex justify-end space-x-4" style="margin: 20px 0;">
                <button onclick="document.getElementById('folderInput').click()" class="bg-gray-500 hover:bg-gray-600 text-white px-4 py-2 rounded">
                    重新选择文件
                </button>
                <button onclick="submitData()" class="bg-blue-500 hover:bg-blue-600 text-white px-4 py-2 rounded">
                    确认提交
                </button>
            </div>
        `;

        // 添加图片预览模态框
        tableHtml += `
            <div id="imageModal" class="fixed inset-0 bg-black bg-opacity-50 hidden flex items-center justify-center z-50">
                <div class="bg-white p-4 rounded-lg max-w-4xl max-h-[90vh] overflow-auto">
                    <img id="previewImage" src="" alt="证书预览" class="max-w-full">
                    <button onclick="closeImageModal()" class="mt-4 bg-gray-500 hover:bg-gray-600 text-white px-4 py-2 rounded">
                        关闭
                    </button>
                </div>
            </div>
        `;

        // 清空并设置新内容
        uploadArea.innerHTML = '';
        console.log('准备插入表格HTML');
        uploadArea.innerHTML = tableHtml;
        console.log('表格HTML已插入');

        // 检查表格是否正确插入
        const table = uploadArea.querySelector('table');
        if (table) {
            console.log('表格元素存在');
            console.log('表格尺寸:', {
                width: table.offsetWidth,
                height: table.offsetHeight,
                rows: table.rows.length
            });
        } else {
            console.error('表格元素未找到');
        }

    } catch (error) {
        console.error('显示数据表格时出错:', error);
        console.error('错误详情:', {
            processedDataSize: processedData.size,
            firstEntry: processedData.values().next().value,
            uploadAreaContent: uploadArea.innerHTML
        });
        showError('显示数据表格时出错: ' + error.message);
        
        // 显示错误信息在页面上
        uploadArea.innerHTML = `
            <div class="bg-red-50 p-4 rounded-lg">
                <p class="text-red-700">显示数据表格时出错: ${error.message}</p>
                <p class="text-sm text-gray-600 mt-2">请检查上传的Excel文件格式是否正确</p>
                <button onclick="document.getElementById('folderInput').click()" class="mt-4 bg-gray-500 hover:bg-gray-600 text-white px-4 py-2 rounded">
                    重新选择文件
                </button>
            </div>
        `;
    }
}

// 图片预览功能
function previewImage(imageUrl) {
    const modal = document.getElementById('imageModal');
    const previewImg = document.getElementById('previewImage');
    previewImg.src = imageUrl;
    modal.classList.remove('hidden');
}

// 关闭图片预览
function closeImageModal() {
    const modal = document.getElementById('imageModal');
    modal.classList.add('hidden');
}

// 显示处理结果
function showProcessingResult(hasErrors, errorMessages = []) {
    const uploadArea = document.getElementById('uploadArea');
    
    if (hasErrors) {
        let html = '<div class="text-center">';
        html += '<p class="text-red-500 mb-4">⚠️ 处理过程中出现错误</p>';
        html += '<div class="text-left mb-4 p-4 bg-red-50 rounded">';
        errorMessages.forEach(msg => {
            html += `<p class="text-red-700 mb-2">• ${msg}</p>`;
        });
        html += '</div>';
        html += '<p class="text-sm text-gray-600 mb-4">请查看控制台获取详细信息</p>';
        html += `
            <button onclick="document.getElementById('folderInput').click()" class="bg-gray-500 hover:bg-gray-600 text-white px-4 py-2 rounded">
                重新选择文件
            </button>
        `;
        html += '</div>';
        uploadArea.innerHTML = html;
    } else {
        try {
            // 将数据转换为可存储的格式
            const dataToStore = Array.from(processedData.entries()).map(([certificateNumber, data]) => {
                return [certificateNumber, {
                    data: data.data,
                    images: data.images.map(img => ({
                        name: img.name,
                        url: img.url,
                        data: img.data
                    }))
                }];
            });

            // 将数据存储到 sessionStorage
            sessionStorage.setItem('schoolUploadData', JSON.stringify(dataToStore));
            
            // 跳转到数据检查页面
            window.location.href = '/school/check';
        } catch (error) {
            console.error('存储数据时出错:', error);
            showError('处理数据时出错: ' + error.message);
        }
    }
}

// 处理文件上传
async function handleFiles(files) {
    try {
        // 显示加载提示
        const uploadArea = document.getElementById('uploadArea');
        uploadArea.innerHTML = '<div class="text-center"><p class="text-gray-600">正在处理文件，请稍候...</p><div class="mt-4 animate-spin rounded-full h-8 w-8 border-t-2 border-b-2 border-blue-500 mx-auto"></div></div>';

        // 清空之前的数据
        processedData.clear();

        // 先处理Excel文件
        const excelFiles = Array.from(files).filter(file => 
            file.name.endsWith('.xlsx') || file.name.endsWith('.xls'));
        const imageFiles = Array.from(files).filter(file => 
            file.type.startsWith('image/'));

        if (excelFiles.length === 0) {
            throw new Error('请至少上传一个Excel文件');
        }

        // 处理Excel文件
        for (const file of excelFiles) {
            await processExcelFile(file);
        }

        // 处理图片文件
        const imageErrors = [];
        await Promise.all(imageFiles.map(file => processImageFile(file).catch(error => {
            console.error(error);
            imageErrors.push(`${file.name}: ${error.message}`);
            return null;
        })));

        // 显示处理结果
        if (imageErrors.length > 0) {
            showProcessingResult(true, imageErrors);
        } else {
            showProcessingResult(false);
        }

    } catch (error) {
        console.error('处理文件时出错:', error);
        showProcessingResult(true, [error.message]);
    }
}

// 设置拖放事件监听器
document.addEventListener('DOMContentLoaded', function() {
    const uploadArea = document.getElementById('uploadArea');
    const folderInput = document.getElementById('folderInput');

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
        await handleFiles(e.dataTransfer.files);
    });

    // 监听文件选择
    folderInput.addEventListener('change', async (e) => {
        if (e.target.files.length > 0) {
            await handleFiles(e.target.files);
        }
    });
}); 