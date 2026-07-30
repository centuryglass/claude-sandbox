/* 
 * File:   MultiReflectionShader.cpp
 * Author: Anthony
 * 
 * Created on May 11, 2013, 2:08 PM
 */

#include "MultiReflectionShader.h"
#include "Ray.h"
#include "TextureMap.h"
#include "AreaLight.h"
using namespace sivelab;
#include <set>
using namespace std;

Vector3D MultiReflectionShader::getHitColor(Scene* scene, HitStruct& hit) {
    Vector3D shade;
    Vector3D rCol(0, 0, 0);
    set<Light*> lights = scene->getLights();

    for (std::set<Light*>::iterator it = lights.begin();
            it != lights.end(); ++it) {
        for (int i = 0; i < reflectionCount; i++) {
            Light * light = *it;

            Vector3D lightPos;
            if (light->getType() == 'a' && hit.getViewingRay().getMaxRayNum() > 1) {
                Ray ray = hit.getViewingRay();
                lightPos = ((AreaLight *) light)->getLightPt(ray);
            } else lightPos = light->getPosition();

            Vector3D l = lightPos - hit.getIntersect();
            Vector3D shadowMult(1, 1, 1);
            if (useShadow) {
                shadowMult = calculateShadowMult(hit, light, l, scene);
            }
            l.normalize();

            float m = hit.getSurfaceNormal().dot(l);
            if (m < 0) m *= -1;
            shade +=
                    (((diffuse->getTexel(hit) * light->getIntensity()) * m)
                    * shadowMult) / reflectionCount;
            if (mirrorCoef > 0) {
                if (hit.getDepth() > 0) {
                    hit.setDepth(hit.getDepth() - 1);
                    Vector3D rCol;
                    if (calculateReflection(hit, scene)) {
                        rCol += (hit.getHitColor(scene));
                    } else rCol += (((diffuse->getTexel(hit) * light->getIntensity()) * m)
                            * shadowMult) / reflectionCount;
                }
            }
        }
        rCol *= mirrorCoef;
        shade *= (1.0 - mirrorCoef);
        shade += rCol;
        if (scene->generatePhotonMap) {
            if (hit.getViewingRay().getRayNum() == 0) {
                Vector3D photon = scene->getLightMap()->getClosest(hit.getIntersect(),
                        scene->getPhotonRadius());
                hit.setPhoton(photon);
                shade += photon;
            } else shade += hit.getViewingRay().getPhoton();

            shade.clamp(0, 1);
            return shade;
        }
    }
}

Vector3D MultiReflectionShader::mapLightRay
(Scene* scene, HitStruct& hit, Vector3D light, int depth) {
    //calculate radiance
    Ray ray = hit.getViewingRay();

    Vector3D shade;
    float m = (ray.getDirection()).dot(hit.getSurfaceNormal());
    if (m < 0) m = 0;
    shade += ((diffuse->getTexel(hit) * light) * m);

    if (mirrorCoef > 0) {
        shade *= (1.0 - mirrorCoef);
        shade += (mirrorCoef * light);
    }
    shade.clamp(0, 1);
    if (ray.getRand() > 0.5 && depth != 0) {
        return shade;
    } else {

        HitStruct subHit = hit;
        if (calculateReflection(subHit, scene)) {
            hit = subHit;
            return subHit.getShape()->getShader()->mapLightRay
                    (scene, hit, shade, depth + 1);
        } else {
            Vector3D miss(-1, -1, -1);
            return miss;
        }
    }
}