from django.shortcuts import render
from imgTest.models import IMG
from PIL import Image
import base64
from io import BytesIO
import os
# Create your views here.

def image_to_base64(image_path):
    img = Image.open(image_path).convert('RGB')
    output_buffer = BytesIO()
    img.save(output_buffer, format='JPEG')
    byte_data = output_buffer.getvalue()
    base64_str = base64.b64encode(byte_data)
    return base64_str

def uploadImg(request): 
    if request.method == 'POST':
        img = IMG(img_url=request.FILES.get('img'))
        img.save()
        img.base64 = image_to_base64(img.img_url.path)
        img.save()
        #im = Image.open(BytesIO(base64.b64decode(img.base64)))
        #im.show()
    return render(request, 'imgupload.html')

def showImg(request):
    imgs = IMG.objects.all()
    context = {
        'imgs' : imgs
    }
    return render(request, 'showImg.html', context)
