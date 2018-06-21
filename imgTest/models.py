from django.db import models
# from django.conf import settings
# Create your models here.

class IMG(models.Model):
    img_url = models.ImageField(upload_to='img')
    base64 = models.TextField(blank=True)
