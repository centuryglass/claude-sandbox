/* 
 * File:   MultiReflectionShader.h
 * Author: Anthony
 *
 * Created on May 11, 2013, 2:08 PM
 */

#ifndef MULTIREFLECTIONSHADER_H
#define	MULTIREFLECTIONSHADER_H
#include "Vector3D.h"
#include "Scene.h"
#include "HitStruct.h"
#include "TextureMap.h"

using namespace sivelab;

class MultiReflectionShader : public Shader {
private:
    TextureMap * diffuse;
    double mirrorCoef,range;
    int reflectionCount;
    bool useShadow;
public:

    /**
     * Instantiate a new lambertian shader
     * @param diffusion diffusion color value
     * @param useShadows set to true to render shadows on this object.
     */
    MultiReflectionShader(TextureMap * diffusion,double mirror,int rCount, bool useShadows) {
        diffuse = diffusion;
        useShadow = useShadows;
        mirrorCoef = mirror;
        reflectionCount = rCount;
        type = "MultiReflection";
    }

 
    ~MultiReflectionShader() {
    }
    /**
     * Given a hitpoint, return the color data for that hit
     * @param scene The scene in which the hit occured.
     * @param hit The hitStructure containing data about the hit
     * @return The calculated color value.
     */
    Vector3D getHitColor(Scene* scene, HitStruct& hit);

    Vector3D mapLightRay(Scene* scene, HitStruct& hit, Vector3D light,int depth);
    /**
     * Gets the shadow cast by an object with this shader
     * @param hit where the shadow ray hits the object
     * @param l the light the object is blocking
     * @param scene
     * @return a Vector3D indicating what percent of the light source passes
     * through the object (always <0,0,0> here)
     */
    Vector3D getShadow(HitStruct &hit, Light * l, Scene * scene) {
        Vector3D s;
        return s;
    }
};

#endif	/* MULTIREFLECTIONSHADER_H */

